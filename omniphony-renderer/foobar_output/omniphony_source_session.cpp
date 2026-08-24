#include "stdafx.h"

#include "omniphony_source_session.h"

#include <deque>
#include <mutex>

namespace {

constexpr std::uint32_t kRequiredSourceAbiMajor = 0;
constexpr std::uint32_t kRequiredSourceAbiMinor = 4;
constexpr std::uint32_t kOutputSampleRate = 48'000;
constexpr std::size_t kMaxQueuedFrames = 2u * kOutputSampleRate;
const char kSourceSessionModuleAnchor = 0;

bool same_config(const OmniphonySourceConfig& left, const OmniphonySourceConfig& right) noexcept {
    return left.sample_rate_hz == right.sample_rate_hz &&
        left.spatial_mode == right.spatial_mode &&
        left.externalization == right.externalization &&
        left.hrir_source == right.hrir_source &&
        left.unit_scale_m == right.unit_scale_m &&
        left.reflection_level == right.reflection_level;
}

bool sample_matches(float expected, float actual) noexcept {
    if (!std::isfinite(expected) || !std::isfinite(actual)) {
        return false;
    }
    const float scale = std::max(std::abs(expected), std::abs(actual));
    return std::abs(expected - actual) <= 1.0e-6f + 1.0e-5f * scale;
}

class SourceBackend {
public:
    ~SourceBackend() {
        close();
    }

    bool matches(const OmniphonySourceConfig& config) const noexcept {
        return processor_ && haveConfig_ && same_config(config_, config);
    }

    bool ensure(const OmniphonySourceConfig& config) noexcept {
        if (matches(config)) {
            return true;
        }
        close();
        if (config.sample_rate_hz != kOutputSampleRate) {
            return false;
        }

        std::array<wchar_t, 32'768> path{};
        HMODULE self = nullptr;
        if (!GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                reinterpret_cast<LPCWSTR>(&kSourceSessionModuleAnchor),
                &self)) {
            return false;
        }
        const DWORD length = GetModuleFileNameW(
            self, path.data(), static_cast<DWORD>(path.size()));
        if (length == 0 || length >= path.size()) {
            return false;
        }
        std::wstring sibling(path.data(), length);
        const auto separator = sibling.find_last_of(L"\\/");
        if (separator == std::wstring::npos) {
            return false;
        }
        sibling.resize(separator + 1);
        sibling.append(L"omniphony_source.dll");

        module_ = LoadLibraryExW(sibling.c_str(), nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
        if (!module_) {
            return false;
        }

        abiMajor_ = resolve<AbiFn>("omniphony_source_abi_major");
        abiMinor_ = resolve<AbiFn>("omniphony_source_abi_minor");
        create_ = resolve<CreateFn>("omniphony_source_create");
        destroy_ = resolve<DestroyFn>("omniphony_source_destroy");
        reset_ = resolve<ResetFn>("omniphony_source_reset");
        setMixBudget_ = resolve<SetMixBudgetFn>("omniphony_source_set_mix_budget");
        processEvents_ = resolve<ProcessEventsFn>("omniphony_source_process_events_f32");
        if (!abiMajor_ || !abiMinor_ || !create_ || !destroy_ || !reset_ ||
            !setMixBudget_ || !processEvents_ ||
            abiMajor_() != kRequiredSourceAbiMajor ||
            abiMinor_() < kRequiredSourceAbiMinor) {
            close();
            return false;
        }

        processor_ = create_(&config);
        if (!processor_) {
            close();
            return false;
        }
        config_ = config;
        haveConfig_ = true;
        return true;
    }

    void close() noexcept {
        if (processor_ && destroy_) {
            destroy_(processor_);
        }
        processor_ = nullptr;
        haveConfig_ = false;
        abiMajor_ = nullptr;
        abiMinor_ = nullptr;
        create_ = nullptr;
        destroy_ = nullptr;
        reset_ = nullptr;
        setMixBudget_ = nullptr;
        processEvents_ = nullptr;
        if (module_) {
            FreeLibrary(module_);
            module_ = nullptr;
        }
    }

    void reset() noexcept {
        if (processor_ && reset_) {
            (void)reset_(processor_);
        }
    }

    std::int32_t render(
        const OmniphonySourceConfig& config,
        const OmniphonySourceMixBudgetV1& mixBudget,
        const float* input,
        const OmniphonySourceEvidenceV1* sources,
        std::size_t sourceCount,
        const OmniphonySourceEvidenceEventV1* events,
        std::size_t eventCount,
        std::size_t frames,
        std::uint64_t samplePos,
        std::uint32_t rampFrames,
        float* output) noexcept {
        if (!ensure(config)) {
            return -20;
        }
        const std::int32_t budgetStatus = setMixBudget_(processor_, &mixBudget);
        if (budgetStatus != 0) {
            return budgetStatus;
        }
        return processEvents_(
            processor_, input, sources, sourceCount, events, eventCount,
            frames, samplePos, rampFrames, output);
    }

private:
    using AbiFn = std::uint32_t (*)();
    using CreateFn = OmniphonySourceProcessor* (*)(const OmniphonySourceConfig*);
    using DestroyFn = void (*)(OmniphonySourceProcessor*);
    using ResetFn = std::int32_t (*)(OmniphonySourceProcessor*);
    using SetMixBudgetFn = std::int32_t (*)(
        OmniphonySourceProcessor*, const OmniphonySourceMixBudgetV1*);
    using ProcessEventsFn = std::int32_t (*)(
        OmniphonySourceProcessor*,
        const float*,
        const OmniphonySourceEvidenceV1*,
        std::size_t,
        const OmniphonySourceEvidenceEventV1*,
        std::size_t,
        std::size_t,
        std::uint64_t,
        std::uint32_t,
        float*);

    template <typename T>
    T resolve(const char* name) noexcept {
        return reinterpret_cast<T>(GetProcAddress(module_, name));
    }

    HMODULE module_ = nullptr;
    OmniphonySourceProcessor* processor_ = nullptr;
    OmniphonySourceConfig config_{};
    bool haveConfig_ = false;
    AbiFn abiMajor_ = nullptr;
    AbiFn abiMinor_ = nullptr;
    CreateFn create_ = nullptr;
    DestroyFn destroy_ = nullptr;
    ResetFn reset_ = nullptr;
    SetMixBudgetFn setMixBudget_ = nullptr;
    ProcessEventsFn processEvents_ = nullptr;
};

struct RenderedPacket {
    std::uint64_t epoch = 0;
    std::uint64_t samplePos = 0;
    std::uint32_t sampleRate = 0;
    std::size_t frames = 0;
    std::size_t readFrame = 0;
    std::vector<float> reference;
    std::vector<float> rendered;
};

class SourceSessionService {
public:
    std::uint32_t outputActive() const noexcept {
        return active_.load(std::memory_order_acquire) ? 1u : 0u;
    }

    void setOutputActive(bool active) noexcept {
        std::lock_guard<std::mutex> lock(mutex_);
        clearQueueUnlocked();
        backend_.close();
        currentEpoch_ = 0;
        active_.store(active, std::memory_order_release);
    }

    void flushOutput() noexcept {
        std::lock_guard<std::mutex> lock(mutex_);
        clearQueueUnlocked();
        backend_.reset();
    }

    std::int32_t reset(std::uint64_t epoch) noexcept {
        std::lock_guard<std::mutex> lock(mutex_);
        clearQueueUnlocked();
        currentEpoch_ = epoch;
        backend_.reset();
        return 0;
    }

    std::int32_t publish(
        const OmniphonySourceConfig* config,
        std::uint64_t epoch,
        const OmniphonySourceMixBudgetV1* mixBudget,
        const float* sourceInput,
        const OmniphonySourceEvidenceV1* sources,
        std::size_t sourceCount,
        const OmniphonySourceEvidenceEventV1* events,
        std::size_t eventCount,
        std::size_t frames,
        std::uint64_t samplePos,
        std::uint32_t rampFrames,
        const float* referenceStereo) noexcept {
        if (!active_.load(std::memory_order_acquire)) {
            return -10;
        }
        if (!config || !mixBudget || !sourceInput || !sources || !referenceStereo ||
            sourceCount == 0 || (eventCount != 0 && !events)) {
            return -11;
        }
        if (config->sample_rate_hz != kOutputSampleRate || frames == 0 ||
            frames > kMaxQueuedFrames || frames > SIZE_MAX / 2u) {
            return -12;
        }

        const std::size_t stereoSamples = frames * 2u;
        RenderedPacket packet{};
        packet.epoch = epoch;
        packet.samplePos = samplePos;
        packet.sampleRate = config->sample_rate_hz;
        packet.frames = frames;
        try {
            packet.reference.assign(referenceStereo, referenceStereo + stereoSamples);
            packet.rendered.resize(stereoSamples);
        } catch (...) {
            return -13;
        }

        std::lock_guard<std::mutex> lock(mutex_);
        if (!active_.load(std::memory_order_acquire)) {
            return -10;
        }
        if (currentEpoch_ != epoch || !backend_.matches(*config)) {
            clearQueueUnlocked();
            backend_.reset();
            currentEpoch_ = epoch;
        }

        const std::int32_t status = backend_.render(
            *config, *mixBudget, sourceInput, sources, sourceCount,
            events, eventCount, frames, samplePos, rampFrames,
            packet.rendered.data());
        if (status != 0) {
            return status;
        }

        if (queuedFrames_ > kMaxQueuedFrames - frames) {
            clearQueueUnlocked();
            backend_.reset();
            return -14;
        }
        queuedFrames_ += frames;
        packets_.push_back(std::move(packet));
        return 0;
    }

    bool tryConsume(
        const float* deliveredStereo,
        float* renderedStereo,
        std::size_t frames,
        std::uint32_t sampleRate) noexcept {
        if (!active_.load(std::memory_order_acquire) || !deliveredStereo ||
            !renderedStereo || frames == 0 || sampleRate != kOutputSampleRate) {
            return false;
        }

        std::unique_lock<std::mutex> lock(mutex_, std::try_to_lock);
        if (!lock.owns_lock() || packets_.empty()) {
            return false;
        }
        if (queuedFrames_ < frames) {
            clearQueueUnlocked();
            backend_.reset();
            return false;
        }

        // First pass validates the complete delivered block without advancing
        // queue state. A mismatch can therefore never produce half-substituted
        // headphone audio.
        std::size_t remaining = frames;
        std::size_t deliveredFrame = 0;
        for (const auto& packet : packets_) {
            if (remaining == 0) {
                break;
            }
            if (packet.sampleRate != sampleRate || packet.readFrame > packet.frames) {
                clearQueueUnlocked();
                backend_.reset();
                return false;
            }
            const std::size_t available = packet.frames - packet.readFrame;
            const std::size_t take = std::min(remaining, available);
            for (std::size_t frame = 0; frame < take; ++frame) {
                const std::size_t expectedBase = (packet.readFrame + frame) * 2u;
                const std::size_t deliveredBase = (deliveredFrame + frame) * 2u;
                if (!sample_matches(packet.reference[expectedBase], deliveredStereo[deliveredBase]) ||
                    !sample_matches(packet.reference[expectedBase + 1u], deliveredStereo[deliveredBase + 1u])) {
                    clearQueueUnlocked();
                    backend_.reset();
                    return false;
                }
            }
            deliveredFrame += take;
            remaining -= take;
        }
        if (remaining != 0) {
            clearQueueUnlocked();
            backend_.reset();
            return false;
        }

        // Second pass commits the exact matching rendered frames.
        remaining = frames;
        std::size_t outputFrame = 0;
        while (remaining != 0 && !packets_.empty()) {
            RenderedPacket& packet = packets_.front();
            const std::size_t available = packet.frames - packet.readFrame;
            const std::size_t take = std::min(remaining, available);
            std::copy_n(
                packet.rendered.data() + packet.readFrame * 2u,
                take * 2u,
                renderedStereo + outputFrame * 2u);
            packet.readFrame += take;
            outputFrame += take;
            remaining -= take;
            queuedFrames_ -= take;
            if (packet.readFrame == packet.frames) {
                packets_.pop_front();
            }
        }
        return remaining == 0;
    }

private:
    void clearQueueUnlocked() noexcept {
        packets_.clear();
        queuedFrames_ = 0;
    }

    std::atomic<bool> active_{false};
    std::mutex mutex_;
    std::deque<RenderedPacket> packets_;
    std::size_t queuedFrames_ = 0;
    std::uint64_t currentEpoch_ = 0;
    SourceBackend backend_;
};

SourceSessionService& source_session() {
    static SourceSessionService service;
    return service;
}

} // namespace

extern "C" __declspec(dllexport) std::uint32_t omniphony_foobar_source_session_abi_major(void) {
    return OMNIPHONY_FOOBAR_SOURCE_SESSION_ABI_MAJOR;
}

extern "C" __declspec(dllexport) std::uint32_t omniphony_foobar_source_session_abi_minor(void) {
    return OMNIPHONY_FOOBAR_SOURCE_SESSION_ABI_MINOR;
}

extern "C" __declspec(dllexport) std::uint32_t omniphony_foobar_source_session_output_active(void) {
    return source_session().outputActive();
}

extern "C" __declspec(dllexport) std::int32_t omniphony_foobar_source_session_reset(
    std::uint64_t session_epoch) {
    return source_session().reset(session_epoch);
}

extern "C" __declspec(dllexport) std::int32_t omniphony_foobar_source_session_publish_v1(
    const OmniphonySourceConfig* config,
    std::uint64_t session_epoch,
    const OmniphonySourceMixBudgetV1* mix_budget,
    const float* source_input,
    const OmniphonySourceEvidenceV1* sources,
    std::size_t source_count,
    const OmniphonySourceEvidenceEventV1* events,
    std::size_t event_count,
    std::size_t frames,
    std::uint64_t sample_pos,
    std::uint32_t ramp_frames,
    const float* reference_stereo) {
    return source_session().publish(
        config, session_epoch, mix_budget, source_input, sources, source_count,
        events, event_count, frames, sample_pos, ramp_frames, reference_stereo);
}

bool omniphony_source_session_try_consume(
    const float* delivered_stereo,
    float* rendered_stereo,
    std::size_t frames,
    std::uint32_t sample_rate_hz) noexcept {
    return source_session().tryConsume(
        delivered_stereo, rendered_stereo, frames, sample_rate_hz);
}

void omniphony_source_session_set_output_active(bool active) noexcept {
    source_session().setOutputActive(active);
}

void omniphony_source_session_flush_output() noexcept {
    source_session().flushOutput();
}
