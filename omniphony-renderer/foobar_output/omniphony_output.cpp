#include "stdafx.h"

#include "../realtime_ffi/include/omniphony_realtime.h"
#include "omniphony_source_session.h"

namespace {

using Microsoft::WRL::ComPtr;

constexpr GUID kOutputGuid = {
    0x95cb7ed1, 0x5c63, 0x4f74, {0x9d, 0xb1, 0x3b, 0x9c, 0x1a, 0xf1, 0xcc, 0x01}};
constexpr GUID kDeviceGuid = {
    0xa7df4e92, 0x91a6, 0x4c8e, {0xa6, 0xcf, 0x7b, 0xe8, 0x5d, 0x8e, 0xc3, 0x01}};

constexpr std::uint32_t kSampleRate = 48'000;
constexpr std::uint32_t kChannels = 2;
const char kModuleAnchor = 0;

void ThrowOutputError(HRESULT hr) {
    if (hr == AUDCLNT_E_DEVICE_INVALIDATED || hr == AUDCLNT_E_RESOURCES_INVALIDATED) {
        throw exception_output_invalidated();
    }
    if (hr == AUDCLNT_E_DEVICE_IN_USE || hr == AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED) {
        throw exception_output_device_in_use();
    }
    if (hr == AUDCLNT_E_UNSUPPORTED_FORMAT) {
        throw exception_output_unsupported_stream_format();
    }
    throw exception_io_data();
}

void Check(HRESULT hr) {
    if (FAILED(hr)) {
        ThrowOutputError(hr);
    }
}

WAVEFORMATEXTENSIBLE StereoFloat48() {
    WAVEFORMATEXTENSIBLE format{};
    format.Format.wFormatTag = WAVE_FORMAT_EXTENSIBLE;
    format.Format.nChannels = kChannels;
    format.Format.nSamplesPerSec = kSampleRate;
    format.Format.wBitsPerSample = 32;
    format.Format.nBlockAlign = kChannels * sizeof(float);
    format.Format.nAvgBytesPerSec = format.Format.nSamplesPerSec * format.Format.nBlockAlign;
    format.Format.cbSize = sizeof(WAVEFORMATEXTENSIBLE) - sizeof(WAVEFORMATEX);
    format.Samples.wValidBitsPerSample = 32;
    format.dwChannelMask = KSAUDIO_SPEAKER_STEREO;
    format.SubFormat = KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
    return format;
}

class RealtimeCurrent {
public:
    ~RealtimeCurrent() {
        close();
    }

    bool open(std::uint32_t sampleRate) noexcept {
        close();

        HMODULE self = nullptr;
        if (!GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                    GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                reinterpret_cast<LPCWSTR>(&kModuleAnchor),
                &self)) {
            return false;
        }

        std::array<wchar_t, 32'768> path{};
        const DWORD length = GetModuleFileNameW(self, path.data(), static_cast<DWORD>(path.size()));
        if (length == 0 || length >= path.size()) {
            return false;
        }
        std::wstring sibling(path.data(), length);
        const auto separator = sibling.find_last_of(L"\\/");
        if (separator == std::wstring::npos) {
            return false;
        }
        sibling.resize(separator + 1);
        sibling.append(L"omniphony_realtime.dll");

        module_ = LoadLibraryExW(sibling.c_str(), nullptr, LOAD_WITH_ALTERED_SEARCH_PATH);
        if (!module_) {
            return false;
        }

        abiMajor_ = resolve<AbiFn>("omniphony_realtime_abi_major");
        abiMinor_ = resolve<AbiFn>("omniphony_realtime_abi_minor");
        create_ = resolve<CreateFn>("omniphony_realtime_create");
        destroy_ = resolve<DestroyFn>("omniphony_realtime_destroy");
        setMode_ = resolve<SetModeFn>("omniphony_realtime_set_mode");
        process_ = resolve<ProcessFn>("omniphony_realtime_process_f32");
        latency_ = resolve<LatencyFn>("omniphony_realtime_latency_frames");
        reset_ = resolve<ResetFn>("omniphony_realtime_reset");
        if (!abiMajor_ || !abiMinor_ || !create_ || !destroy_ || !setMode_ ||
            !process_ || !latency_ || !reset_ || abiMajor_() != OMNIPHONY_REALTIME_ABI_MAJOR ||
            abiMinor_() < OMNIPHONY_REALTIME_ABI_MINOR) {
            close();
            return false;
        }

        const OmniphonyRealtimeConfig config{sampleRate, kChannels};
        processor_ = create_(&config);
        if (!processor_ || setMode_(processor_, OMNIPHONY_REALTIME_MODE_CURRENT) != 0) {
            close();
            return false;
        }
        return true;
    }

    void close() noexcept {
        if (processor_ && destroy_) {
            destroy_(processor_);
        }
        processor_ = nullptr;
        abiMajor_ = nullptr;
        abiMinor_ = nullptr;
        create_ = nullptr;
        destroy_ = nullptr;
        setMode_ = nullptr;
        process_ = nullptr;
        latency_ = nullptr;
        reset_ = nullptr;
        // omniphony_realtime.dll deliberately pins itself for detached worker
        // safety. Release only this loader reference; the module stays resident.
        if (module_) {
            FreeLibrary(module_);
            module_ = nullptr;
        }
    }

    bool process(const float* input, float* output, std::size_t frames) noexcept {
        return processor_ && process_ &&
            process_(processor_, input, output, frames) == 0;
    }

    std::size_t latencyFrames() const noexcept {
        return processor_ && latency_ ? latency_(processor_) : 0;
    }

    void reset() noexcept {
        if (processor_ && reset_) {
            (void)reset_(processor_);
        }
    }

private:
    using AbiFn = std::uint32_t (*)();
    using CreateFn = OmniphonyRealtimeProcessor* (*)(const OmniphonyRealtimeConfig*);
    using DestroyFn = void (*)(OmniphonyRealtimeProcessor*);
    using SetModeFn = std::int32_t (*)(OmniphonyRealtimeProcessor*, std::uint32_t);
    using ProcessFn = std::int32_t (*)(
        OmniphonyRealtimeProcessor*, const float*, float*, std::size_t);
    using LatencyFn = std::size_t (*)(const OmniphonyRealtimeProcessor*);
    using ResetFn = std::int32_t (*)(OmniphonyRealtimeProcessor*);

    template <typename T>
    T resolve(const char* name) noexcept {
        return reinterpret_cast<T>(GetProcAddress(module_, name));
    }

    HMODULE module_ = nullptr;
    OmniphonyRealtimeProcessor* processor_ = nullptr;
    AbiFn abiMajor_ = nullptr;
    AbiFn abiMinor_ = nullptr;
    CreateFn create_ = nullptr;
    DestroyFn destroy_ = nullptr;
    SetModeFn setMode_ = nullptr;
    ProcessFn process_ = nullptr;
    LatencyFn latency_ = nullptr;
    ResetFn reset_ = nullptr;
};

class OmniphonyOutput : public output_impl {
public:
    OmniphonyOutput(const GUID&, double bufferLength, bool, t_uint32)
        : bufferLengthSeconds_(std::clamp(bufferLength, 0.01, 2.0)) {
        const HRESULT init = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
        if (SUCCEEDED(init)) {
            comInitialized_ = true;
        } else if (init != RPC_E_CHANGED_MODE) {
            Check(init);
        }
        // Foobar creates the selected output object before it necessarily has a
        // decoded stream spec. Advertise source-session ownership here so the
        // first decoder block cannot pre-binauralize and then enter Current.
        omniphony_source_session_set_output_active(true);
    }

    ~OmniphonyOutput() {
        closeEndpoint();
        if (comInitialized_) {
            CoUninitialize();
        }
    }

    static void g_enum_devices(output_device_enum_callback& callback) {
        constexpr char name[] = "Omniphony";
        callback.on_device(kDeviceGuid, name, sizeof(name) - 1);
    }

    static GUID g_get_guid() { return kOutputGuid; }
    static const char* g_get_name() { return "Output"; }
    static bool g_advanced_settings_query() { return false; }
    static bool g_needs_bitdepth_config() { return false; }
    static bool g_needs_dither_config() { return false; }
    static bool g_needs_device_list_prefixes() { return false; }
    static bool g_supports_multiple_streams() { return false; }
    static bool g_is_high_latency() { return false; }
    static std::uint32_t g_extra_flags() { return 0; }

    unsigned get_forced_sample_rate() override { return kSampleRate; }
    unsigned get_forced_channel_mask() override { return audio_chunk::channel_config_stereo; }

    pfc::eventHandle_t get_trigger_event() override {
        return event_;
    }

    bool is_progressing() override {
        return started_ && !paused_;
    }

    void pause(bool state) override {
        paused_ = state;
        if (!client_) return;
        if (state) {
            if (started_) {
                Check(client_->Stop());
                started_ = false;
            }
        } else if (haveWritten_) {
            startIfNeeded();
        }
    }

    void volume_set(double db) override {
        const double bounded = std::clamp(db, -150.0, 0.0);
        volumeGain_.store(
            static_cast<float>(std::pow(10.0, bounded / 20.0)),
            std::memory_order_release);
    }

protected:
    void on_update() override {
        writableFrames_ = 0;
        if (!client_) return;
        UINT32 padding = 0;
        Check(client_->GetCurrentPadding(&padding));
        if (padding > bufferFrames_) {
            throw exception_io_data();
        }
        writableFrames_ = bufferFrames_ - padding;
    }

    t_size can_write_samples() override {
        return writableFrames_;
    }

    t_size get_latency_samples() override {
        if (!client_) return lastWriteUsedSourceSession_ ? 0 : current_.latencyFrames();
        UINT32 padding = 0;
        Check(client_->GetCurrentPadding(&padding));
        const std::size_t renderLatency =
            lastWriteUsedSourceSession_ ? 0u : current_.latencyFrames();
        return static_cast<t_size>(padding) + renderLatency;
    }

    void on_flush() override {
        if (client_) {
            if (started_) {
                Check(client_->Stop());
            }
            started_ = false;
            Check(client_->Reset());
        }
        omniphony_source_session_flush_output();
        current_.reset();
        lastWriteUsedSourceSession_ = false;
        writableFrames_ = bufferFrames_;
        haveWritten_ = false;
    }

    void open(const audio_chunk::spec_t& spec) override {
        if (spec.sampleRate != kSampleRate || spec.chanCount != kChannels ||
            spec.chanMask != audio_chunk::channel_config_stereo) {
            throw exception_output_unsupported_stream_format();
        }

        // Preserve source packets decoded after output construction but before
        // this first physical-open call. Reopens do close/clear the old session.
        if (client_.Get() != nullptr || render_.Get() != nullptr ||
            device_.Get() != nullptr || event_ != nullptr) {
            closeEndpoint();
        }

        ComPtr<IMMDeviceEnumerator> enumerator;
        Check(CoCreateInstance(
            __uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
            IID_PPV_ARGS(enumerator.ReleaseAndGetAddressOf())));
        Check(enumerator->GetDefaultAudioEndpoint(
            eRender, eMultimedia, device_.ReleaseAndGetAddressOf()));
        Check(device_->Activate(
            __uuidof(IAudioClient2), CLSCTX_ALL, nullptr,
            reinterpret_cast<void**>(client_.ReleaseAndGetAddressOf())));

        AudioClientProperties properties{};
        properties.cbSize = sizeof(properties);
        properties.bIsOffload = FALSE;
        properties.eCategory = AudioCategory_Media;
        properties.Options = AUDCLNT_STREAMOPTIONS_RAW;
        Check(client_->SetClientProperties(&properties));

        auto format = StereoFloat48();
        WAVEFORMATEX* closest = nullptr;
        const HRESULT supported = client_->IsFormatSupported(
            AUDCLNT_SHAREMODE_SHARED, &format.Format, &closest);
        if (closest) CoTaskMemFree(closest);
        if (supported != S_OK) {
            ThrowOutputError(
                supported == S_FALSE ? AUDCLNT_E_UNSUPPORTED_FORMAT : supported);
        }

        event_ = CreateEventW(nullptr, FALSE, FALSE, nullptr);
        if (!event_) {
            throw exception_io_data();
        }

        const REFERENCE_TIME bufferDuration = static_cast<REFERENCE_TIME>(
            bufferLengthSeconds_ * 10'000'000.0);
        Check(client_->Initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_NOPERSIST,
            bufferDuration,
            0,
            &format.Format,
            nullptr));
        Check(client_->SetEventHandle(event_));
        Check(client_->GetBufferSize(&bufferFrames_));
        Check(client_->GetService(IID_PPV_ARGS(render_.ReleaseAndGetAddressOf())));

        inputScratch_.assign(static_cast<std::size_t>(bufferFrames_) * kChannels, 0.0f);
        scratch_.assign(static_cast<std::size_t>(bufferFrames_) * kChannels, 0.0f);
        writableFrames_ = bufferFrames_;
        if (!current_.open(kSampleRate)) {
            throw exception_output_device_not_found();
        }
        lastWriteUsedSourceSession_ = false;
        omniphony_source_session_set_output_active(true);
    }

    void write(const audio_chunk& data) override {
        const std::size_t frames = data.get_sample_count();
        if (!render_ || data.get_channels() != kChannels || data.get_srate() != kSampleRate ||
            frames > writableFrames_ || frames > bufferFrames_) {
            throw exception_io_data();
        }

        const audio_sample* input = data.get_data();
        if (!input && frames != 0) {
            throw exception_io_data();
        }
        const std::size_t samples = frames * kChannels;
        for (std::size_t sample = 0; sample < samples; ++sample) {
            inputScratch_[sample] = static_cast<float>(input[sample]);
        }

        float* processed = scratch_.data();
        const bool usedSourceSession = omniphony_source_session_try_consume(
            inputScratch_.data(), processed, frames, kSampleRate);
        if (!usedSourceSession) {
            // Current did not process source-session blocks. Reset it before the
            // first ordinary-stereo block after a source scene so stale room or
            // inference history can never leak across routing modes.
            if (lastWriteUsedSourceSession_) {
                current_.reset();
            }
            if (!current_.process(inputScratch_.data(), processed, frames)) {
                std::copy_n(inputScratch_.data(), samples, processed);
            }
        }
        lastWriteUsedSourceSession_ = usedSourceSession;

        const float gain = volumeGain_.load(std::memory_order_acquire);
        if (gain != 1.0f) {
            for (std::size_t sample = 0; sample < samples; ++sample) {
                processed[sample] *= gain;
            }
        }

        BYTE* endpoint = nullptr;
        Check(render_->GetBuffer(static_cast<UINT32>(frames), &endpoint));
        if (!endpoint && frames != 0) {
            (void)render_->ReleaseBuffer(0, 0);
            throw exception_io_data();
        }
        std::memcpy(endpoint, processed, frames * kChannels * sizeof(float));
        Check(render_->ReleaseBuffer(static_cast<UINT32>(frames), 0));
        writableFrames_ -= static_cast<UINT32>(frames);
        haveWritten_ = true;
        if (!paused_) {
            startIfNeeded();
        }
    }

    void on_force_play() override {
        if (haveWritten_ && !paused_) {
            startIfNeeded();
        }
    }

private:
    void startIfNeeded() {
        if (!started_ && client_) {
            Check(client_->Start());
            started_ = true;
        }
    }

    void closeEndpoint() noexcept {
        omniphony_source_session_set_output_active(false);
        if (client_ && started_) {
            (void)client_->Stop();
        }
        started_ = false;
        haveWritten_ = false;
        lastWriteUsedSourceSession_ = false;
        writableFrames_ = 0;
        bufferFrames_ = 0;
        inputScratch_.clear();
        scratch_.clear();
        current_.close();
        render_.Reset();
        client_.Reset();
        device_.Reset();
        if (event_) {
            CloseHandle(event_);
            event_ = nullptr;
        }
    }

    double bufferLengthSeconds_ = 0.1;
    bool comInitialized_ = false;
    bool started_ = false;
    bool paused_ = false;
    bool haveWritten_ = false;
    bool lastWriteUsedSourceSession_ = false;
    UINT32 bufferFrames_ = 0;
    UINT32 writableFrames_ = 0;
    std::atomic<float> volumeGain_{1.0f};
    std::vector<float> inputScratch_;
    std::vector<float> scratch_;
    RealtimeCurrent current_;
    ComPtr<IMMDevice> device_;
    ComPtr<IAudioClient2> client_;
    ComPtr<IAudioRenderClient> render_;
    HANDLE event_ = nullptr;
};

output_factory_t<OmniphonyOutput> g_omniphonyOutputFactory;

} // namespace
