#include <windows.h>
#include <unknwn.h>
#include <audioenginebaseapo.h>
#include <audioengineextensionapo.h>
#include <audiomediatype.h>
#include <BaseAudioProcessingObject.h>
#include <ksmedia.h>

#include "omniphony_realtime.h"
#include "OmniphonyStreamFallback.h"

#include <atomic>
#include <cstring>
#include <limits>
#include <new>
#include <string>
#include <vector>

namespace {

constexpr GUID kOmniphonyStreamApoClsid = {
    0x07d403d9, 0x8a98, 0x43ef, {0x8c, 0x28, 0x86, 0x51, 0x75, 0x6d, 0x83, 0xbe}};

constexpr DWORD kStereoMask = SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT;
constexpr DWORD kSevenOneMask =
    SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT | SPEAKER_FRONT_CENTER |
    SPEAKER_LOW_FREQUENCY | SPEAKER_BACK_LEFT | SPEAKER_BACK_RIGHT |
    SPEAKER_SIDE_LEFT | SPEAKER_SIDE_RIGHT;

HINSTANCE g_module = nullptr;
volatile LONG g_factoryLocks = 0;

bool ReadAudioFormat(IAudioMediaType* mediaType, UNCOMPRESSEDAUDIOFORMAT& format) noexcept {
    format = {};
    return mediaType && SUCCEEDED(mediaType->GetUncompressedAudioFormat(&format));
}

bool IsFloat32Format(const UNCOMPRESSEDAUDIOFORMAT& format) noexcept {
    return IsEqualGUID(format.guidFormatType, KSDATAFORMAT_SUBTYPE_IEEE_FLOAT) &&
           format.dwBytesPerSampleContainer == sizeof(float) &&
           format.dwValidBitsPerSample == 32 &&
           format.fFramesPerSecond > 0.0f;
}

bool IsSupportedInputFormat(const UNCOMPRESSEDAUDIOFORMAT& format) noexcept {
    if (!IsFloat32Format(format) || format.dwSamplesPerFrame == 0 ||
        format.dwSamplesPerFrame > 18) {
        return false;
    }
    return format.dwSamplesPerFrame == 2 || format.dwChannelMask != 0;
}

bool IsSupportedOutputFormat(const UNCOMPRESSEDAUDIOFORMAT& format) noexcept {
    return IsFloat32Format(format) && format.dwSamplesPerFrame == 2;
}

bool IsSupportedFormatPair(
    const UNCOMPRESSEDAUDIOFORMAT& input,
    const UNCOMPRESSEDAUDIOFORMAT& output) noexcept {
    return IsSupportedInputFormat(input) && IsSupportedOutputFormat(output) &&
           input.fFramesPerSecond == output.fFramesPerSecond;
}

bool IsRawBypassFormat(const UNCOMPRESSEDAUDIOFORMAT& format) noexcept {
    return IsFloat32Format(format) && format.dwSamplesPerFrame == 2;
}

bool IsRawBypassPair(
    const UNCOMPRESSEDAUDIOFORMAT& input,
    const UNCOMPRESSEDAUDIOFORMAT& output) noexcept {
    return IsRawBypassFormat(input) && IsRawBypassFormat(output) &&
           input.fFramesPerSecond == output.fFramesPerSecond &&
           input.dwBytesPerSampleContainer == output.dwBytesPerSampleContainer &&
           input.dwValidBitsPerSample == output.dwValidBitsPerSample &&
           IsEqualGUID(input.guidFormatType, output.guidFormatType) &&
           input.dwChannelMask == output.dwChannelMask;
}

HRESULT PassThroughMediaType(
    IAudioMediaType* source,
    IAudioMediaType** target) noexcept {
    if (!target) return E_POINTER;
    *target = nullptr;
    if (!source) return E_POINTER;
    source->AddRef();
    *target = source;
    return S_OK;
}

HRESULT CreatePreferredMediaType(
    const UNCOMPRESSEDAUDIOFORMAT& basis,
    UINT32 channels,
    DWORD channelMask,
    IAudioMediaType** mediaType) noexcept {
    if (!mediaType) return E_POINTER;
    *mediaType = nullptr;

    UNCOMPRESSEDAUDIOFORMAT preferred = basis;
    preferred.dwSamplesPerFrame = channels;
    preferred.dwChannelMask = channelMask;
    return CreateAudioMediaTypeFromUncompressedAudioFormat(&preferred, mediaType);
}

class INonDelegatingUnknown {
public:
    virtual HRESULT STDMETHODCALLTYPE NonDelegatingQueryInterface(REFIID riid, void** object) = 0;
    virtual ULONG STDMETHODCALLTYPE NonDelegatingAddRef() = 0;
    virtual ULONG STDMETHODCALLTYPE NonDelegatingRelease() = 0;
};

class RealtimeBridge final {
public:
    RealtimeBridge() = default;
    RealtimeBridge(const RealtimeBridge&) = delete;
    RealtimeBridge& operator=(const RealtimeBridge&) = delete;

    ~RealtimeBridge() { shutdown(); }

    bool start(UINT32 sampleRateHz, UINT32 inputChannels, UINT32 channelMask) noexcept {
        shutdown();
        if (sampleRateHz == 0 || inputChannels == 0 || !g_module) {
            return false;
        }

        wchar_t modulePath[MAX_PATH] = {};
        const DWORD length = GetModuleFileNameW(g_module, modulePath, MAX_PATH);
        if (length == 0 || length >= MAX_PATH) {
            return false;
        }
        std::wstring realtimePath(modulePath, length);
        const size_t separator = realtimePath.find_last_of(L"\\/");
        if (separator == std::wstring::npos) {
            return false;
        }
        realtimePath.resize(separator + 1);
        realtimePath.append(L"omniphony_realtime.dll");

        module_ = LoadLibraryW(realtimePath.c_str());
        if (!module_) {
            return false;
        }

        abiMajor_ = resolve<AbiFn>("omniphony_realtime_abi_major");
        abiMinor_ = resolve<AbiFn>("omniphony_realtime_abi_minor");
        stereoCreate_ = resolve<StereoCreateFn>("omniphony_realtime_create");
        stereoDestroy_ = resolve<StereoDestroyFn>("omniphony_realtime_destroy");
        stereoSetMode_ = resolve<StereoSetModeFn>("omniphony_realtime_set_mode");
        stereoLatency_ = resolve<StereoLatencyFn>("omniphony_realtime_latency_frames");
        stereoProcess_ = resolve<StereoProcessFn>("omniphony_realtime_process_f32");
        bedCreate_ = resolve<BedCreateFn>("omniphony_native_bed_create");
        bedDestroy_ = resolve<BedDestroyFn>("omniphony_native_bed_destroy");
        bedLatency_ = resolve<BedLatencyFn>("omniphony_native_bed_latency_frames");
        bedProcess_ = resolve<BedProcessFn>("omniphony_native_bed_process_f32");

        if (!abiMajor_ || !abiMinor_ || !stereoCreate_ || !stereoDestroy_ ||
            !stereoSetMode_ || !stereoLatency_ || !stereoProcess_ || !bedCreate_ ||
            !bedDestroy_ || !bedLatency_ || !bedProcess_) {
            shutdown();
            return false;
        }
        if (abiMajor_() != OMNIPHONY_REALTIME_ABI_MAJOR ||
            abiMinor_() < OMNIPHONY_REALTIME_ABI_MINOR) {
            shutdown();
            return false;
        }

        if (inputChannels == 2) {
            const OmniphonyRealtimeConfig config{sampleRateHz, inputChannels};
            stereo_ = stereoCreate_(&config);
            if (!stereo_ || stereoSetMode_(stereo_, OMNIPHONY_REALTIME_MODE_CURRENT) != 0) {
                shutdown();
                return false;
            }
            mode_ = Mode::StereoCurrent;
            latencyFrames_ = stereoLatency_(stereo_);
        } else {
            const OmniphonyNativeBedConfig config{sampleRateHz, inputChannels, channelMask};
            bed_ = bedCreate_(&config);
            if (!bed_) {
                shutdown();
                return false;
            }
            mode_ = Mode::NativeBed;
            latencyFrames_ = bedLatency_(bed_);
        }

        if (latencyFrames_ > static_cast<size_t>(std::numeric_limits<HNSTIME>::max() / 10'000'000LL)) {
            shutdown();
            return false;
        }
        latencyHns_ = static_cast<HNSTIME>(
            (static_cast<unsigned long long>(latencyFrames_) * 10'000'000ULL) /
            static_cast<unsigned long long>(sampleRateHz));
        closing_.store(false, std::memory_order_release);
        return true;
    }

    bool process(const float* input, float* output, size_t frames) const noexcept {
        if (closing_.load(std::memory_order_acquire) || !input || !output) {
            return false;
        }
        activeCalls_.fetch_add(1, std::memory_order_acq_rel);
        if (closing_.load(std::memory_order_acquire)) {
            activeCalls_.fetch_sub(1, std::memory_order_acq_rel);
            return false;
        }

        const Mode mode = mode_;
        auto* stereo = stereo_;
        auto* bed = bed_;
        const auto stereoProcess = stereoProcess_;
        const auto bedProcess = bedProcess_;
        bool processed = false;
        if (mode == Mode::StereoCurrent && stereo && stereoProcess) {
            processed = stereoProcess(stereo, input, output, frames) == 0;
        } else if (mode == Mode::NativeBed && bed && bedProcess) {
            processed = bedProcess(bed, input, output, frames) == 0;
        }
        activeCalls_.fetch_sub(1, std::memory_order_acq_rel);
        return processed;
    }

    HNSTIME latencyHns() const noexcept { return latencyHns_; }

    void shutdown() noexcept {
        // Configuration/destruction may run while a final AudioDG callback is
        // returning. Close admission first, then retire the processor only
        // after every already-admitted nonblocking process call has left.
        closing_.store(true, std::memory_order_release);
        while (activeCalls_.load(std::memory_order_acquire) != 0) {
            Sleep(1);
        }
        if (stereo_ && stereoDestroy_) {
            stereoDestroy_(stereo_);
        }
        if (bed_ && bedDestroy_) {
            bedDestroy_(bed_);
        }
        stereo_ = nullptr;
        bed_ = nullptr;
        mode_ = Mode::None;
        latencyFrames_ = 0;
        latencyHns_ = 0;
        abiMajor_ = nullptr;
        abiMinor_ = nullptr;
        stereoCreate_ = nullptr;
        stereoDestroy_ = nullptr;
        stereoSetMode_ = nullptr;
        stereoLatency_ = nullptr;
        stereoProcess_ = nullptr;
        bedCreate_ = nullptr;
        bedDestroy_ = nullptr;
        bedLatency_ = nullptr;
        bedProcess_ = nullptr;
        if (module_) {
            FreeLibrary(module_);
            module_ = nullptr;
        }
    }

private:
    enum class Mode { None, StereoCurrent, NativeBed };
    using AbiFn = uint32_t (*)();
    using StereoCreateFn = OmniphonyRealtimeProcessor* (*)(const OmniphonyRealtimeConfig*);
    using StereoDestroyFn = void (*)(OmniphonyRealtimeProcessor*);
    using StereoSetModeFn = int32_t (*)(OmniphonyRealtimeProcessor*, uint32_t);
    using StereoLatencyFn = size_t (*)(const OmniphonyRealtimeProcessor*);
    using StereoProcessFn = int32_t (*)(OmniphonyRealtimeProcessor*, const float*, float*, size_t);
    using BedCreateFn = OmniphonyNativeBedProcessor* (*)(const OmniphonyNativeBedConfig*);
    using BedDestroyFn = void (*)(OmniphonyNativeBedProcessor*);
    using BedLatencyFn = size_t (*)(const OmniphonyNativeBedProcessor*);
    using BedProcessFn = int32_t (*)(OmniphonyNativeBedProcessor*, const float*, float*, size_t);

    template <typename T>
    T resolve(const char* name) const noexcept {
        return module_ ? reinterpret_cast<T>(GetProcAddress(module_, name)) : nullptr;
    }

    HMODULE module_ = nullptr;
    Mode mode_ = Mode::None;
    OmniphonyRealtimeProcessor* stereo_ = nullptr;
    OmniphonyNativeBedProcessor* bed_ = nullptr;
    size_t latencyFrames_ = 0;
    HNSTIME latencyHns_ = 0;
    AbiFn abiMajor_ = nullptr;
    AbiFn abiMinor_ = nullptr;
    StereoCreateFn stereoCreate_ = nullptr;
    StereoDestroyFn stereoDestroy_ = nullptr;
    StereoSetModeFn stereoSetMode_ = nullptr;
    StereoLatencyFn stereoLatency_ = nullptr;
    StereoProcessFn stereoProcess_ = nullptr;
    BedCreateFn bedCreate_ = nullptr;
    BedDestroyFn bedDestroy_ = nullptr;
    BedLatencyFn bedLatency_ = nullptr;
    BedProcessFn bedProcess_ = nullptr;
    mutable std::atomic<uint32_t> activeCalls_{0};
    std::atomic<bool> closing_{true};
};

class OmniphonyStreamAPO final : public CBaseAudioProcessingObject,
                                 public IAudioSystemEffects,
                                 public IAudioProcessingObjectPreferredFormatSupport,
                                 public INonDelegatingUnknown {
public:
    static volatile LONG instanceCount;
    static const CRegAPOProperties<1> registration;

    explicit OmniphonyStreamAPO(IUnknown* outer)
        : CBaseAudioProcessingObject(registration),
          outer_(outer ? outer : reinterpret_cast<IUnknown*>(static_cast<INonDelegatingUnknown*>(this))) {
        InterlockedIncrement(&instanceCount);
    }

    ~OmniphonyStreamAPO() override {
        resetProcessing();
        InterlockedDecrement(&instanceCount);
    }

    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** object) override {
        return outer_->QueryInterface(riid, object);
    }

    ULONG STDMETHODCALLTYPE AddRef() override { return outer_->AddRef(); }
    ULONG STDMETHODCALLTYPE Release() override { return outer_->Release(); }

    HRESULT STDMETHODCALLTYPE NonDelegatingQueryInterface(REFIID riid, void** object) override {
        if (!object) return E_POINTER;
        *object = nullptr;
        if (IsEqualIID(riid, IID_IUnknown)) {
            *object = static_cast<INonDelegatingUnknown*>(this);
        } else if (IsEqualIID(riid, __uuidof(IAudioProcessingObject))) {
            *object = static_cast<IAudioProcessingObject*>(this);
        } else if (IsEqualIID(riid, __uuidof(IAudioProcessingObjectRT))) {
            *object = static_cast<IAudioProcessingObjectRT*>(this);
        } else if (IsEqualIID(riid, __uuidof(IAudioProcessingObjectConfiguration))) {
            *object = static_cast<IAudioProcessingObjectConfiguration*>(this);
        } else if (IsEqualIID(riid, __uuidof(IAudioSystemEffects))) {
            *object = static_cast<IAudioSystemEffects*>(this);
        } else if (IsEqualIID(riid, __uuidof(IAudioProcessingObjectPreferredFormatSupport))) {
            *object = static_cast<IAudioProcessingObjectPreferredFormatSupport*>(this);
        } else {
            return E_NOINTERFACE;
        }
        reinterpret_cast<IUnknown*>(*object)->AddRef();
        return S_OK;
    }

    ULONG STDMETHODCALLTYPE NonDelegatingAddRef() override {
        return static_cast<ULONG>(InterlockedIncrement(&references_));
    }

    ULONG STDMETHODCALLTYPE NonDelegatingRelease() override {
        const LONG value = InterlockedDecrement(&references_);
        if (value == 0) {
            delete this;
            return 0;
        }
        return static_cast<ULONG>(value);
    }

    HRESULT STDMETHODCALLTYPE Initialize(UINT32 dataSize, BYTE* data) override {
        if ((data == nullptr) != (dataSize == 0)) return E_INVALIDARG;
        if (dataSize != sizeof(APOInitSystemEffects) &&
            dataSize != sizeof(APOInitSystemEffects2) &&
            dataSize != sizeof(APOInitSystemEffects3)) {
            return E_INVALIDARG;
        }
        if (m_bIsInitialized) return HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS);

        GUID processingMode = AUDIO_SIGNALPROCESSINGMODE_DEFAULT;
        if (dataSize == sizeof(APOInitSystemEffects3)) {
            processingMode = reinterpret_cast<APOInitSystemEffects3*>(data)->AudioProcessingMode;
        } else if (dataSize == sizeof(APOInitSystemEffects2)) {
            processingMode = reinterpret_cast<APOInitSystemEffects2*>(data)->AudioProcessingMode;
        }
        rawBypass_ = IsEqualGUID(processingMode, AUDIO_SIGNALPROCESSINGMODE_RAW);
        m_bIsInitialized = true;
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE GetPreferredInputFormat(
        IAudioMediaType* outputFormat,
        IAudioMediaType** preferredFormat) override {
        if (!preferredFormat) return E_POINTER;
        *preferredFormat = nullptr;
        if (!outputFormat) return E_POINTER;

        UNCOMPRESSEDAUDIOFORMAT output = {};
        if (!ReadAudioFormat(outputFormat, output)) {
            return APOERR_FORMAT_NOT_SUPPORTED;
        }
        if (rawBypass_) {
            if (!IsRawBypassFormat(output)) {
                return APOERR_FORMAT_NOT_SUPPORTED;
            }
            return PassThroughMediaType(outputFormat, preferredFormat);
        }
        if (!IsSupportedOutputFormat(output)) {
            return APOERR_FORMAT_NOT_SUPPORTED;
        }

        // Windows 11 23H2+ explicitly supports this headphone-virtualization
        // contract: a stereo-rendering endpoint can have an APO request 7.1
        // upstream. Omniphony then preserves the authored speaker bed and owns
        // the only binaural reduction before the physical stereo DAC.
        return CreatePreferredMediaType(output, 8, kSevenOneMask, preferredFormat);
    }

    HRESULT STDMETHODCALLTYPE GetPreferredOutputFormat(
        IAudioMediaType* inputFormat,
        IAudioMediaType** preferredFormat) override {
        if (!preferredFormat) return E_POINTER;
        *preferredFormat = nullptr;
        if (!inputFormat) return E_POINTER;

        UNCOMPRESSEDAUDIOFORMAT input = {};
        if (!ReadAudioFormat(inputFormat, input)) {
            return APOERR_FORMAT_NOT_SUPPORTED;
        }
        if (rawBypass_) {
            if (!IsRawBypassFormat(input)) {
                return APOERR_FORMAT_NOT_SUPPORTED;
            }
            return PassThroughMediaType(inputFormat, preferredFormat);
        }
        if (!IsSupportedInputFormat(input)) {
            return APOERR_FORMAT_NOT_SUPPORTED;
        }

        // The physical headphone endpoint remains stereo regardless of how rich
        // the authored input bed is. Keep sample rate/container fidelity from
        // the supplied input and prefer only a channel-count reduction here.
        return CreatePreferredMediaType(input, 2, kStereoMask, preferredFormat);
    }

    HRESULT STDMETHODCALLTYPE IsInputFormatSupported(
        IAudioMediaType* oppositeFormat,
        IAudioMediaType* requestedInputFormat,
        IAudioMediaType** supportedInputFormat) override {
        if (supportedInputFormat) *supportedInputFormat = nullptr;
        UNCOMPRESSEDAUDIOFORMAT inputFormat = {};
        if (!ReadAudioFormat(requestedInputFormat, inputFormat)) {
            return requestedInputFormat ? APOERR_FORMAT_NOT_SUPPORTED : E_POINTER;
        }
        if (rawBypass_) {
            if (!IsRawBypassFormat(inputFormat)) {
                return APOERR_FORMAT_NOT_SUPPORTED;
            }
            if (oppositeFormat) {
                UNCOMPRESSEDAUDIOFORMAT outputFormat = {};
                if (!ReadAudioFormat(oppositeFormat, outputFormat) ||
                    !IsRawBypassPair(inputFormat, outputFormat)) {
                    return APOERR_FORMAT_NOT_SUPPORTED;
                }
            }
        } else {
            if (!IsSupportedInputFormat(inputFormat)) {
                return APOERR_FORMAT_NOT_SUPPORTED;
            }
            if (oppositeFormat) {
                UNCOMPRESSEDAUDIOFORMAT outputFormat = {};
                if (!ReadAudioFormat(oppositeFormat, outputFormat) ||
                    !IsSupportedFormatPair(inputFormat, outputFormat)) {
                    return APOERR_FORMAT_NOT_SUPPORTED;
                }
            }
        }
        if (supportedInputFormat) {
            requestedInputFormat->AddRef();
            *supportedInputFormat = requestedInputFormat;
        }
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE IsOutputFormatSupported(
        IAudioMediaType* oppositeFormat,
        IAudioMediaType* requestedOutputFormat,
        IAudioMediaType** supportedOutputFormat) override {
        if (supportedOutputFormat) *supportedOutputFormat = nullptr;
        UNCOMPRESSEDAUDIOFORMAT outputFormat = {};
        if (!ReadAudioFormat(requestedOutputFormat, outputFormat)) {
            return requestedOutputFormat ? APOERR_FORMAT_NOT_SUPPORTED : E_POINTER;
        }
        if (rawBypass_) {
            if (!IsRawBypassFormat(outputFormat)) {
                return APOERR_FORMAT_NOT_SUPPORTED;
            }
            if (oppositeFormat) {
                UNCOMPRESSEDAUDIOFORMAT inputFormat = {};
                if (!ReadAudioFormat(oppositeFormat, inputFormat) ||
                    !IsRawBypassPair(inputFormat, outputFormat)) {
                    return APOERR_FORMAT_NOT_SUPPORTED;
                }
            }
        } else {
            if (!IsSupportedOutputFormat(outputFormat)) {
                return APOERR_FORMAT_NOT_SUPPORTED;
            }
            if (oppositeFormat) {
                UNCOMPRESSEDAUDIOFORMAT inputFormat = {};
                if (!ReadAudioFormat(oppositeFormat, inputFormat) ||
                    !IsSupportedFormatPair(inputFormat, outputFormat)) {
                    return APOERR_FORMAT_NOT_SUPPORTED;
                }
            }
        }
        if (supportedOutputFormat) {
            requestedOutputFormat->AddRef();
            *supportedOutputFormat = requestedOutputFormat;
        }
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE LockForProcess(
        UINT32 inputCount,
        APO_CONNECTION_DESCRIPTOR** inputs,
        UINT32 outputCount,
        APO_CONNECTION_DESCRIPTOR** outputs) override {
        resetProcessing();
        if (inputCount != 1 || outputCount != 1 || !inputs || !outputs ||
            !inputs[0] || !outputs[0] || !inputs[0]->pFormat || !outputs[0]->pFormat) {
            return APOERR_NUM_CONNECTIONS_INVALID;
        }

        UNCOMPRESSEDAUDIOFORMAT inputFormat = {};
        UNCOMPRESSEDAUDIOFORMAT outputFormat = {};
        const bool formatsReadable =
            ReadAudioFormat(inputs[0]->pFormat, inputFormat) &&
            ReadAudioFormat(outputs[0]->pFormat, outputFormat);
        const bool pairSupported = formatsReadable &&
            (rawBypass_
                ? IsRawBypassPair(inputFormat, outputFormat)
                : IsSupportedFormatPair(inputFormat, outputFormat));
        if (!pairSupported) {
            return APOERR_FORMAT_NOT_SUPPORTED;
        }

        const HRESULT hr = CBaseAudioProcessingObject::LockForProcess(
            inputCount, inputs, outputCount, outputs);
        if (FAILED(hr)) return hr;

        inputChannels_ = inputFormat.dwSamplesPerFrame;
        inputChannelMask_ = inputFormat.dwChannelMask;
        outputChannels_ = outputFormat.dwSamplesPerFrame;
        inputBytesPerFrame_ = static_cast<size_t>(inputChannels_) * sizeof(float);
        outputBytesPerFrame_ = static_cast<size_t>(outputChannels_) * sizeof(float);

        // RAW mode is an identity transform. Do not allocate a worker-facing
        // scratch lane and, most importantly, do not load Current. This is the
        // path intended for already-rendered provider egress.
        if (rawBypass_) {
            return S_OK;
        }

        const auto sampleRateHz = static_cast<UINT32>(inputFormat.fFramesPerSecond + 0.5f);
        const size_t maxFrames = inputs[0]->u32MaxFrameCount;
        if (maxFrames > std::numeric_limits<size_t>::max() / inputChannels_) {
            CBaseAudioProcessingObject::UnlockForProcess();
            resetProcessing();
            return E_OUTOFMEMORY;
        }
        try {
            silentInput_.assign(maxFrames * inputChannels_, 0.0f);
        } catch (...) {
            CBaseAudioProcessingObject::UnlockForProcess();
            resetProcessing();
            return E_OUTOFMEMORY;
        }

        if (!realtime_.start(sampleRateHz, inputChannels_, inputFormat.dwChannelMask)) {
            CBaseAudioProcessingObject::UnlockForProcess();
            resetProcessing();
            return APOERR_FORMAT_NOT_SUPPORTED;
        }
        InterlockedExchange(&realtimeEligible_, 1);
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE UnlockForProcess() override {
        resetProcessing();
        return CBaseAudioProcessingObject::UnlockForProcess();
    }

    HRESULT STDMETHODCALLTYPE GetLatency(HNSTIME* latency) override {
        if (!latency) return E_POINTER;
        *latency = InterlockedCompareExchange(&realtimeEligible_, 0, 0) != 0
            ? realtime_.latencyHns()
            : 0;
        return S_OK;
    }

    void STDMETHODCALLTYPE APOProcess(
        UINT32 inputCount,
        APO_CONNECTION_PROPERTY** inputs,
        UINT32 outputCount,
        APO_CONNECTION_PROPERTY** outputs) override {
        if (inputCount == 0 || outputCount == 0 || !inputs || !outputs ||
            !inputs[0] || !outputs[0]) {
            return;
        }

        auto* input = inputs[0];
        auto* output = outputs[0];
        const UINT32 frames = input->u32ValidFrameCount;
        if (inputBytesPerFrame_ == 0 || outputBytesPerFrame_ == 0) {
            output->u32BufferFlags = BUFFER_INVALID;
            output->u32ValidFrameCount = 0;
            return;
        }

        auto* outputBuffer = reinterpret_cast<float*>(output->pBuffer);
        const size_t outputSamples = static_cast<size_t>(frames) * outputChannels_;
        const size_t outputBytes = outputSamples * sizeof(float);
        if (!outputBuffer && outputBytes != 0) {
            output->u32BufferFlags = BUFFER_INVALID;
            output->u32ValidFrameCount = 0;
            return;
        }

        if (rawBypass_) {
            if (inputChannels_ != outputChannels_ || inputBytesPerFrame_ != outputBytesPerFrame_) {
                output->u32BufferFlags = BUFFER_INVALID;
                output->u32ValidFrameCount = 0;
                return;
            }
            switch (input->u32BufferFlags) {
            case BUFFER_VALID: {
                auto* inputBuffer = reinterpret_cast<const float*>(input->pBuffer);
                if (!inputBuffer && outputBytes != 0) {
                    output->u32BufferFlags = BUFFER_INVALID;
                    output->u32ValidFrameCount = 0;
                    return;
                }
                if (output->pBuffer != input->pBuffer && outputBytes != 0) {
                    std::memmove(outputBuffer, inputBuffer, outputBytes);
                }
                output->u32BufferFlags = BUFFER_VALID;
                break;
            }
            case BUFFER_SILENT:
                if (outputBuffer && outputBytes != 0) {
                    std::memset(outputBuffer, 0, outputBytes);
                }
                output->u32BufferFlags = BUFFER_SILENT;
                break;
            default:
                output->u32BufferFlags = BUFFER_INVALID;
                output->u32ValidFrameCount = 0;
                return;
            }
            output->u32ValidFrameCount = frames;
            return;
        }

        if (static_cast<size_t>(frames) > silentInput_.size() / inputChannels_) {
            output->u32BufferFlags = BUFFER_INVALID;
            output->u32ValidFrameCount = 0;
            return;
        }

        const float* processInput = nullptr;
        switch (input->u32BufferFlags) {
        case BUFFER_VALID:
            processInput = reinterpret_cast<const float*>(input->pBuffer);
            if (!processInput && frames != 0) {
                output->u32BufferFlags = BUFFER_INVALID;
                output->u32ValidFrameCount = 0;
                return;
            }
            break;
        case BUFFER_SILENT:
            processInput = silentInput_.data();
            break;
        default:
            output->u32BufferFlags = BUFFER_INVALID;
            output->u32ValidFrameCount = 0;
            return;
        }

        bool processed = false;
        if (frames != 0 && InterlockedCompareExchange(&realtimeEligible_, 0, 0) != 0) {
            processed = realtime_.process(processInput, outputBuffer, frames);
            if (!processed) InterlockedExchange(&realtimeEligible_, 0);
        } else if (frames == 0) {
            processed = true;
        }

        if (!processed) {
            if (inputChannels_ == 2 && input->u32BufferFlags == BUFFER_VALID &&
                output->pBuffer != input->pBuffer && outputBytes != 0) {
                std::memmove(outputBuffer, processInput, outputBytes);
                output->u32BufferFlags = BUFFER_VALID;
            } else if (inputChannels_ > 2 && input->u32BufferFlags == BUFFER_VALID &&
                       processInput && outputBuffer && output->pBuffer != input->pBuffer) {
                omniphony::SafetyFoldDown(
                    processInput,
                    outputBuffer,
                    frames,
                    inputChannels_,
                    inputChannelMask_);
                output->u32BufferFlags = BUFFER_VALID;
            } else {
                if (outputBuffer && outputBytes != 0) std::memset(outputBuffer, 0, outputBytes);
                output->u32BufferFlags = BUFFER_SILENT;
            }
        } else {
            output->u32BufferFlags = BUFFER_VALID;
        }
        output->u32ValidFrameCount = frames;
    }

    STDMETHODIMP GetEffectsList(LPGUID* effects, UINT* effectCount, HANDLE eventHandle) {
        UNREFERENCED_PARAMETER(eventHandle);
        if (!effects || !effectCount) return E_POINTER;
        *effects = nullptr;
        *effectCount = 0;
        return S_OK;
    }

private:
    void resetProcessing() noexcept {
        InterlockedExchange(&realtimeEligible_, 0);
        inputChannels_ = 0;
        inputChannelMask_ = 0;
        outputChannels_ = 0;
        inputBytesPerFrame_ = 0;
        outputBytesPerFrame_ = 0;
        silentInput_.clear();
        realtime_.shutdown();
    }

    volatile LONG references_ = 1;
    volatile LONG realtimeEligible_ = 0;
    UINT32 inputChannels_ = 0;
    DWORD inputChannelMask_ = 0;
    UINT32 outputChannels_ = 0;
    size_t inputBytesPerFrame_ = 0;
    size_t outputBytesPerFrame_ = 0;
    std::vector<float> silentInput_;
    bool rawBypass_ = false;
    IUnknown* outer_ = nullptr;
    RealtimeBridge realtime_;
};

volatile LONG OmniphonyStreamAPO::instanceCount = 0;
#pragma warning(disable : 4815)
const CRegAPOProperties<1> OmniphonyStreamAPO::registration(
    kOmniphonyStreamApoClsid,
    L"Omniphony Stream APO",
    L"Omniphony",
    1,
    0,
    __uuidof(IAudioProcessingObject),
    static_cast<APO_FLAG>(APO_FLAG_BITSPERSAMPLE_MUST_MATCH));

class ApoClassFactory final : public IClassFactory {
public:
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** object) override {
        if (!object) return E_POINTER;
        *object = nullptr;
        if (IsEqualIID(riid, IID_IUnknown) || IsEqualIID(riid, IID_IClassFactory)) {
            *object = static_cast<IClassFactory*>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }
    ULONG STDMETHODCALLTYPE AddRef() override {
        return static_cast<ULONG>(InterlockedIncrement(&references_));
    }
    ULONG STDMETHODCALLTYPE Release() override {
        const LONG value = InterlockedDecrement(&references_);
        if (value == 0) { delete this; return 0; }
        return static_cast<ULONG>(value);
    }
    HRESULT STDMETHODCALLTYPE CreateInstance(IUnknown* outer, REFIID riid, void** object) override {
        if (!object) return E_POINTER;
        *object = nullptr;
        if (outer && !IsEqualIID(riid, IID_IUnknown)) return CLASS_E_NOAGGREGATION;
        auto* apo = new (std::nothrow) OmniphonyStreamAPO(outer);
        if (!apo) return E_OUTOFMEMORY;
        const HRESULT hr = apo->NonDelegatingQueryInterface(riid, object);
        apo->NonDelegatingRelease();
        return hr;
    }
    HRESULT STDMETHODCALLTYPE LockServer(BOOL lock) override {
        if (lock) InterlockedIncrement(&g_factoryLocks);
        else InterlockedDecrement(&g_factoryLocks);
        return S_OK;
    }
private:
    volatile LONG references_ = 1;
};

std::wstring GuidText(REFGUID guid) {
    wchar_t text[64] = {};
    StringFromGUID2(guid, text, 64);
    return text;
}

HRESULT WriteString(HKEY key, const wchar_t* name, const std::wstring& value) {
    const LSTATUS status = RegSetValueExW(
        key, name, 0, REG_SZ, reinterpret_cast<const BYTE*>(value.c_str()),
        static_cast<DWORD>((value.size() + 1) * sizeof(wchar_t)));
    return HRESULT_FROM_WIN32(status);
}

HRESULT RegisterComClass() {
    wchar_t modulePath[MAX_PATH] = {};
    if (!GetModuleFileNameW(g_module, modulePath, MAX_PATH)) {
        return HRESULT_FROM_WIN32(GetLastError());
    }
    const std::wstring clsid = GuidText(kOmniphonyStreamApoClsid);
    const std::wstring path = L"SOFTWARE\\Classes\\CLSID\\" + clsid;
    HKEY classKey = nullptr;
    LSTATUS status = RegCreateKeyExW(
        HKEY_LOCAL_MACHINE, path.c_str(), 0, nullptr, 0, KEY_WRITE, nullptr,
        &classKey, nullptr);
    if (status != ERROR_SUCCESS) return HRESULT_FROM_WIN32(status);
    HRESULT hr = WriteString(classKey, nullptr, L"Omniphony Stream APO");
    RegCloseKey(classKey);
    if (FAILED(hr)) return hr;

    HKEY serverKey = nullptr;
    status = RegCreateKeyExW(
        HKEY_LOCAL_MACHINE, (path + L"\\InprocServer32").c_str(), 0, nullptr, 0,
        KEY_WRITE, nullptr, &serverKey, nullptr);
    if (status != ERROR_SUCCESS) return HRESULT_FROM_WIN32(status);
    hr = WriteString(serverKey, nullptr, modulePath);
    if (SUCCEEDED(hr)) hr = WriteString(serverKey, L"ThreadingModel", L"Both");
    RegCloseKey(serverKey);
    return hr;
}

void UnregisterComClass() {
    const std::wstring path = L"SOFTWARE\\Classes\\CLSID\\" + GuidText(kOmniphonyStreamApoClsid);
    RegDeleteTreeW(HKEY_LOCAL_MACHINE, path.c_str());
}

} // namespace

STDAPI DllGetClassObject(REFCLSID clsid, REFIID riid, LPVOID* object) {
    if (!object) return E_POINTER;
    *object = nullptr;
    if (!IsEqualCLSID(clsid, kOmniphonyStreamApoClsid)) return CLASS_E_CLASSNOTAVAILABLE;
    auto* factory = new (std::nothrow) ApoClassFactory();
    if (!factory) return E_OUTOFMEMORY;
    const HRESULT hr = factory->QueryInterface(riid, object);
    factory->Release();
    return hr;
}

STDAPI DllCanUnloadNow() {
    return OmniphonyStreamAPO::instanceCount == 0 && g_factoryLocks == 0 ? S_OK : S_FALSE;
}

STDAPI DllRegisterServer() {
    HRESULT hr = RegisterAPO(OmniphonyStreamAPO::registration);
    if (FAILED(hr)) return hr;
    hr = RegisterComClass();
    if (FAILED(hr)) UnregisterAPO(kOmniphonyStreamApoClsid);
    return hr;
}

STDAPI DllUnregisterServer() {
    UnregisterComClass();
    return UnregisterAPO(kOmniphonyStreamApoClsid);
}

BOOL WINAPI DllMain(HINSTANCE module, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        g_module = module;
        DisableThreadLibraryCalls(module);
    }
    return TRUE;
}
