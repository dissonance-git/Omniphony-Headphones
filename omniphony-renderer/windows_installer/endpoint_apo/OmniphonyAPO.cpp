#include <windows.h>
#include <unknwn.h>
#include <audioenginebaseapo.h>
#include <audioengineextensionapo.h>
#include <BaseAudioProcessingObject.h>
#include <ksmedia.h>

#include "omniphony_realtime.h"

#include <atomic>
#include <cstring>
#include <limits>
#include <new>
#include <string>

namespace {

constexpr GUID kOmniphonyApoClsid = {
    0xa9333bfe, 0x39c1, 0x40fd, {0xb4, 0xb0, 0xec, 0xc5, 0x91, 0x41, 0x0b, 0x47}};

HINSTANCE g_module = nullptr;
volatile LONG g_factoryLocks = 0;

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

    ~RealtimeBridge() {
        shutdown();
    }

    bool start(UINT32 sampleRateHz, UINT32 channels) noexcept {
        shutdown();
        if (sampleRateHz == 0 || channels != 2 || !g_module) {
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
        create_ = resolve<CreateFn>("omniphony_realtime_create");
        destroy_ = resolve<DestroyFn>("omniphony_realtime_destroy");
        setMode_ = resolve<SetModeFn>("omniphony_realtime_set_mode");
        latencyFrames_ = resolve<LatencyFramesFn>("omniphony_realtime_latency_frames");
        process_ = resolve<ProcessFn>("omniphony_realtime_process_f32");
        if (!abiMajor_ || !abiMinor_ || !create_ || !destroy_ || !setMode_ ||
            !latencyFrames_ || !process_) {
            shutdown();
            return false;
        }
        if (abiMajor_() != OMNIPHONY_REALTIME_ABI_MAJOR ||
            abiMinor_() < OMNIPHONY_REALTIME_ABI_MINOR) {
            shutdown();
            return false;
        }

        const OmniphonyRealtimeConfig config{sampleRateHz, channels};
        processor_ = create_(&config);
        if (!processor_) {
            shutdown();
            return false;
        }
        if (setMode_(processor_, OMNIPHONY_REALTIME_MODE_CURRENT) != 0) {
            shutdown();
            return false;
        }

        const size_t latencyFrames = latencyFrames_(processor_);
        if (latencyFrames > static_cast<size_t>(std::numeric_limits<HNSTIME>::max() / 10'000'000LL)) {
            shutdown();
            return false;
        }
        latencyHns_ = static_cast<HNSTIME>(
            (static_cast<unsigned long long>(latencyFrames) * 10'000'000ULL) /
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

        auto* processor = processor_;
        const auto process = process_;
        const bool processed = processor && process && process(processor, input, output, frames) == 0;
        activeCalls_.fetch_sub(1, std::memory_order_acq_rel);
        return processed;
    }

    HNSTIME latencyHns() const noexcept {
        return latencyHns_;
    }

    void shutdown() noexcept {
        // Configuration/destruction may run while a final AudioDG callback is
        // returning. Close admission first, then retire the processor only
        // after every already-admitted nonblocking process call has left.
        closing_.store(true, std::memory_order_release);
        while (activeCalls_.load(std::memory_order_acquire) != 0) {
            Sleep(1);
        }
        if (processor_ && destroy_) {
            destroy_(processor_);
        }
        processor_ = nullptr;
        latencyHns_ = 0;
        abiMajor_ = nullptr;
        abiMinor_ = nullptr;
        create_ = nullptr;
        destroy_ = nullptr;
        setMode_ = nullptr;
        latencyFrames_ = nullptr;
        process_ = nullptr;
        if (module_) {
            FreeLibrary(module_);
            module_ = nullptr;
        }
    }

private:
    using AbiFn = uint32_t (*)();
    using CreateFn = OmniphonyRealtimeProcessor* (*)(const OmniphonyRealtimeConfig*);
    using DestroyFn = void (*)(OmniphonyRealtimeProcessor*);
    using SetModeFn = int32_t (*)(OmniphonyRealtimeProcessor*, uint32_t);
    using LatencyFramesFn = size_t (*)(const OmniphonyRealtimeProcessor*);
    using ProcessFn = int32_t (*)(OmniphonyRealtimeProcessor*, const float*, float*, size_t);

    template <typename T>
    T resolve(const char* name) const noexcept {
        return module_ ? reinterpret_cast<T>(GetProcAddress(module_, name)) : nullptr;
    }

    HMODULE module_ = nullptr;
    OmniphonyRealtimeProcessor* processor_ = nullptr;
    HNSTIME latencyHns_ = 0;
    AbiFn abiMajor_ = nullptr;
    AbiFn abiMinor_ = nullptr;
    CreateFn create_ = nullptr;
    DestroyFn destroy_ = nullptr;
    SetModeFn setMode_ = nullptr;
    LatencyFramesFn latencyFrames_ = nullptr;
    ProcessFn process_ = nullptr;
    mutable std::atomic<uint32_t> activeCalls_{0};
    std::atomic<bool> closing_{true};
};

class OmniphonyAPO final : public CBaseAudioProcessingObject,
                           public IAudioSystemEffects,
                           public INonDelegatingUnknown {
public:
    static volatile LONG instanceCount;
    static const CRegAPOProperties<1> registration;

    explicit OmniphonyAPO(IUnknown* outer)
        : CBaseAudioProcessingObject(registration),
          outer_(outer ? outer : reinterpret_cast<IUnknown*>(static_cast<INonDelegatingUnknown*>(this))) {
        InterlockedIncrement(&instanceCount);
    }

    ~OmniphonyAPO() override {
        InterlockedExchange(&realtimeEligible_, 0);
        bytesPerFrame_ = 0;
        realtime_.shutdown();
        InterlockedDecrement(&instanceCount);
    }

    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** object) override {
        return outer_->QueryInterface(riid, object);
    }

    ULONG STDMETHODCALLTYPE AddRef() override {
        return outer_->AddRef();
    }

    ULONG STDMETHODCALLTYPE Release() override {
        return outer_->Release();
    }

    HRESULT STDMETHODCALLTYPE NonDelegatingQueryInterface(REFIID riid, void** object) override {
        if (!object) {
            return E_POINTER;
        }
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
        if ((data == nullptr) != (dataSize == 0)) {
            return E_INVALIDARG;
        }
        if (dataSize != sizeof(APOInitSystemEffects) &&
            dataSize != sizeof(APOInitSystemEffects2) &&
            dataSize != sizeof(APOInitSystemEffects3)) {
            return E_INVALIDARG;
        }
        if (m_bIsInitialized) {
            return HRESULT_FROM_WIN32(ERROR_ALREADY_EXISTS);
        }

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

    HRESULT STDMETHODCALLTYPE LockForProcess(
        UINT32 inputCount,
        APO_CONNECTION_DESCRIPTOR** inputs,
        UINT32 outputCount,
        APO_CONNECTION_DESCRIPTOR** outputs) override {
        InterlockedExchange(&realtimeEligible_, 0);
        bytesPerFrame_ = 0;
        realtime_.shutdown();

        const HRESULT hr = CBaseAudioProcessingObject::LockForProcess(
            inputCount, inputs, outputCount, outputs);
        if (FAILED(hr)) {
            return hr;
        }

        if (inputCount != 1 || outputCount != 1 || !inputs || !outputs ||
            !inputs[0] || !outputs[0] || !inputs[0]->pFormat || !outputs[0]->pFormat) {
            return S_OK;
        }

        UNCOMPRESSEDAUDIOFORMAT inputFormat = {};
        UNCOMPRESSEDAUDIOFORMAT outputFormat = {};
        if (FAILED(inputs[0]->pFormat->GetUncompressedAudioFormat(&inputFormat)) ||
            FAILED(outputs[0]->pFormat->GetUncompressedAudioFormat(&outputFormat))) {
            return S_OK;
        }

        if (inputFormat.dwSamplesPerFrame == 0 ||
            inputFormat.dwSamplesPerFrame != outputFormat.dwSamplesPerFrame ||
            inputFormat.dwBytesPerSampleContainer == 0 ||
            inputFormat.dwBytesPerSampleContainer != outputFormat.dwBytesPerSampleContainer ||
            inputFormat.fFramesPerSecond <= 0.0f ||
            inputFormat.fFramesPerSecond != outputFormat.fFramesPerSecond) {
            return S_OK;
        }

        const size_t channels = inputFormat.dwSamplesPerFrame;
        const size_t bytesPerSample = inputFormat.dwBytesPerSampleContainer;
        if (channels > std::numeric_limits<size_t>::max() / bytesPerSample) {
            return S_OK;
        }
        bytesPerFrame_ = channels * bytesPerSample;

        // RAW is the provider-egress escape hatch. Microsoft system-effect
        // samples keep SFX processing transparent in this mode. Omniphony must
        // do the same so already-rendered binaural stereo can reach the physical
        // endpoint without being passed through Current a second time.
        if (rawBypass_) {
            return S_OK;
        }

        const bool float32 =
            IsEqualGUID(inputFormat.guidFormatType, KSDATAFORMAT_SUBTYPE_IEEE_FLOAT) &&
            IsEqualGUID(outputFormat.guidFormatType, KSDATAFORMAT_SUBTYPE_IEEE_FLOAT) &&
            inputFormat.dwBytesPerSampleContainer == sizeof(float) &&
            outputFormat.dwBytesPerSampleContainer == sizeof(float) &&
            inputFormat.dwValidBitsPerSample == 32 &&
            outputFormat.dwValidBitsPerSample == 32;
        if (!float32) {
            return S_OK;
        }

        const auto sampleRateHz = static_cast<UINT32>(inputFormat.fFramesPerSecond + 0.5f);
        if (realtime_.start(sampleRateHz, inputFormat.dwSamplesPerFrame)) {
            InterlockedExchange(&realtimeEligible_, 1);
        }
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE UnlockForProcess() override {
        InterlockedExchange(&realtimeEligible_, 0);
        bytesPerFrame_ = 0;
        realtime_.shutdown();
        return CBaseAudioProcessingObject::UnlockForProcess();
    }

    HRESULT STDMETHODCALLTYPE GetLatency(HNSTIME* latency) override {
        if (!latency) {
            return E_POINTER;
        }
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
        if (inputCount == 0 || outputCount == 0 || !inputs || !outputs || !inputs[0] || !outputs[0]) {
            return;
        }

        auto* input = inputs[0];
        auto* output = outputs[0];
        const UINT32 frames = input->u32ValidFrameCount;
        if (bytesPerFrame_ == 0 ||
            static_cast<size_t>(frames) > std::numeric_limits<size_t>::max() / bytesPerFrame_) {
            output->u32BufferFlags = BUFFER_INVALID;
            output->u32ValidFrameCount = 0;
            return;
        }
        const size_t bytes = static_cast<size_t>(frames) * bytesPerFrame_;
        auto* inputBuffer = reinterpret_cast<const void*>(input->pBuffer);
        auto* outputBuffer = reinterpret_cast<void*>(output->pBuffer);

        switch (input->u32BufferFlags) {
        case BUFFER_VALID: {
            if ((!inputBuffer || !outputBuffer) && bytes != 0) {
                output->u32BufferFlags = BUFFER_INVALID;
                output->u32ValidFrameCount = 0;
                break;
            }

            bool processed = false;
            if (frames != 0 &&
                InterlockedCompareExchange(&realtimeEligible_, 0, 0) != 0) {
                processed = realtime_.process(
                    static_cast<const float*>(inputBuffer),
                    static_cast<float*>(outputBuffer),
                    frames);
                if (!processed) {
                    InterlockedExchange(&realtimeEligible_, 0);
                }
            }

            if (!processed && output->pBuffer != input->pBuffer && bytes != 0) {
                std::memmove(outputBuffer, inputBuffer, bytes);
            }
            output->u32BufferFlags = BUFFER_VALID;
            output->u32ValidFrameCount = frames;
            break;
        }
        case BUFFER_SILENT: {
            if (!outputBuffer && bytes != 0) {
                output->u32BufferFlags = BUFFER_INVALID;
                output->u32ValidFrameCount = 0;
                break;
            }

            if (outputBuffer && bytes != 0) {
                std::memset(outputBuffer, 0, bytes);
            }

            bool processed = false;
            if (frames != 0 && outputBuffer &&
                InterlockedCompareExchange(&realtimeEligible_, 0, 0) != 0) {
                // BUFFER_SILENT does not guarantee readable input memory. Feed the
                // pre-zeroed output buffer in-place so Current keeps its worker
                // timeline and can emit delayed tails safely.
                processed = realtime_.process(
                    static_cast<const float*>(outputBuffer),
                    static_cast<float*>(outputBuffer),
                    frames);
                if (!processed) {
                    InterlockedExchange(&realtimeEligible_, 0);
                }
            }

            output->u32BufferFlags = processed ? BUFFER_VALID : BUFFER_SILENT;
            output->u32ValidFrameCount = frames;
            break;
        }
        default:
            output->u32BufferFlags = BUFFER_INVALID;
            output->u32ValidFrameCount = 0;
            break;
        }
    }

    STDMETHODIMP GetEffectsList(LPGUID* effects, UINT* effectCount, HANDLE eventHandle) {
        UNREFERENCED_PARAMETER(eventHandle);
        if (!effects || !effectCount) {
            return E_POINTER;
        }
        *effects = nullptr;
        *effectCount = 0;
        return S_OK;
    }

private:
    volatile LONG references_ = 1;
    volatile LONG realtimeEligible_ = 0;
    size_t bytesPerFrame_ = 0;
    bool rawBypass_ = false;
    IUnknown* outer_ = nullptr;
    RealtimeBridge realtime_;
};

volatile LONG OmniphonyAPO::instanceCount = 0;
#pragma warning(disable : 4815)
const CRegAPOProperties<1> OmniphonyAPO::registration(
    kOmniphonyApoClsid,
    L"Omniphony Endpoint APO",
    L"Omniphony downstream fork",
    1,
    0,
    __uuidof(IAudioProcessingObject),
    static_cast<APO_FLAG>(APO_FLAG_FRAMESPERSECOND_MUST_MATCH |
                          APO_FLAG_BITSPERSAMPLE_MUST_MATCH |
                          APO_FLAG_SAMPLESPERFRAME_MUST_MATCH |
                          APO_FLAG_INPLACE));

class ApoClassFactory final : public IClassFactory {
public:
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** object) override {
        if (!object) {
            return E_POINTER;
        }
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
        if (value == 0) {
            delete this;
            return 0;
        }
        return static_cast<ULONG>(value);
    }

    HRESULT STDMETHODCALLTYPE CreateInstance(IUnknown* outer, REFIID riid, void** object) override {
        if (!object) {
            return E_POINTER;
        }
        *object = nullptr;
        if (outer && !IsEqualIID(riid, IID_IUnknown)) {
            return CLASS_E_NOAGGREGATION;
        }
        auto* apo = new (std::nothrow) OmniphonyAPO(outer);
        if (!apo) {
            return E_OUTOFMEMORY;
        }
        const HRESULT hr = apo->NonDelegatingQueryInterface(riid, object);
        apo->NonDelegatingRelease();
        return hr;
    }

    HRESULT STDMETHODCALLTYPE LockServer(BOOL lock) override {
        if (lock) {
            InterlockedIncrement(&g_factoryLocks);
        } else {
            InterlockedDecrement(&g_factoryLocks);
        }
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

    const std::wstring clsid = GuidText(kOmniphonyApoClsid);
    const std::wstring path = L"SOFTWARE\\Classes\\CLSID\\" + clsid;
    HKEY classKey = nullptr;
    LSTATUS status = RegCreateKeyExW(HKEY_LOCAL_MACHINE, path.c_str(), 0, nullptr, 0, KEY_WRITE, nullptr, &classKey, nullptr);
    if (status != ERROR_SUCCESS) {
        return HRESULT_FROM_WIN32(status);
    }
    HRESULT hr = WriteString(classKey, nullptr, L"Omniphony Endpoint APO");
    RegCloseKey(classKey);
    if (FAILED(hr)) {
        return hr;
    }

    HKEY serverKey = nullptr;
    status = RegCreateKeyExW(HKEY_LOCAL_MACHINE, (path + L"\\InprocServer32").c_str(), 0, nullptr, 0, KEY_WRITE, nullptr, &serverKey, nullptr);
    if (status != ERROR_SUCCESS) {
        return HRESULT_FROM_WIN32(status);
    }
    hr = WriteString(serverKey, nullptr, modulePath);
    if (SUCCEEDED(hr)) {
        hr = WriteString(serverKey, L"ThreadingModel", L"Both");
    }
    RegCloseKey(serverKey);
    return hr;
}

void UnregisterComClass() {
    const std::wstring path = L"SOFTWARE\\Classes\\CLSID\\" + GuidText(kOmniphonyApoClsid);
    RegDeleteTreeW(HKEY_LOCAL_MACHINE, path.c_str());
}

} // namespace

STDAPI DllGetClassObject(REFCLSID clsid, REFIID riid, LPVOID* object) {
    if (!object) {
        return E_POINTER;
    }
    *object = nullptr;
    if (!IsEqualCLSID(clsid, kOmniphonyApoClsid)) {
        return CLASS_E_CLASSNOTAVAILABLE;
    }
    auto* factory = new (std::nothrow) ApoClassFactory();
    if (!factory) {
        return E_OUTOFMEMORY;
    }
    const HRESULT hr = factory->QueryInterface(riid, object);
    factory->Release();
    return hr;
}

STDAPI DllCanUnloadNow() {
    return OmniphonyAPO::instanceCount == 0 && g_factoryLocks == 0 ? S_OK : S_FALSE;
}

STDAPI DllRegisterServer() {
    HRESULT hr = RegisterAPO(OmniphonyAPO::registration);
    if (FAILED(hr)) {
        return hr;
    }
    hr = RegisterComClass();
    if (FAILED(hr)) {
        UnregisterAPO(kOmniphonyApoClsid);
    }
    return hr;
}

STDAPI DllUnregisterServer() {
    UnregisterComClass();
    return UnregisterAPO(kOmniphonyApoClsid);
}

BOOL WINAPI DllMain(HINSTANCE module, DWORD reason, LPVOID) {
    if (reason == DLL_PROCESS_ATTACH) {
        g_module = module;
        DisableThreadLibraryCalls(module);
    }
    return TRUE;
}
