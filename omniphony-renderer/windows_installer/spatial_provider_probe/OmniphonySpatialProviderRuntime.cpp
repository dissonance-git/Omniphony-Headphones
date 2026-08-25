#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <avrt.h>
#include <mmreg.h>
#include <spatialaudioclient.h>

#include <atomic>
#include <cstdint>
#include <cstring>
#include <memory>
#include <mutex>
#include <new>
#include <system_error>
#include <thread>

#include "OmniphonySpatialProviderRuntime.h"
#include "OmniphonySpatialRawOutputPump.h"
#include "OmniphonySpatialRealtimeBridge.h"
#include "OmniphonySpatialStereoQueue.h"

namespace {

constexpr std::size_t kProviderQueueFrames = 480u * 8u;
constexpr LONG kSourcePeriodMilliseconds = 10;

HRESULT LastErrorOrFail() noexcept {
    const DWORD error = GetLastError();
    return HRESULT_FROM_WIN32(error == ERROR_SUCCESS ? ERROR_GEN_FAILURE : error);
}

bool ValidStaticActivationBlob(
    const PROPVARIANT* activationParams,
    SpatialAudioObjectRenderStreamActivationParams& params) noexcept {
    if (!activationParams ||
        activationParams->vt != VT_BLOB ||
        activationParams->blob.cbSize != sizeof(SpatialAudioObjectRenderStreamActivationParams) ||
        !activationParams->blob.pBlobData) {
        return false;
    }

    std::memcpy(&params, activationParams->blob.pBlobData, sizeof(params));
    return params.EventHandle != nullptr &&
           params.MinDynamicObjectCount == 0 &&
           params.MaxDynamicObjectCount == 0;
}

class ProviderStaticRenderStream final : public ISpatialAudioObjectRenderStream {
public:
    ProviderStaticRenderStream(
        ISpatialAudioObjectRenderStream* inner,
        std::shared_ptr<OmniphonySpatialStereoQueue> queue,
        HANDLE clientEvent) noexcept
        : inner_(inner),
          queue_(std::move(queue)),
          clientEvent_(clientEvent) {
        OmniphonySpatialProviderModuleAddRef();
    }

    ~ProviderStaticRenderStream() {
        (void)Stop();
        pump_.Close();
        if (inner_) {
            inner_->Release();
            inner_ = nullptr;
        }
        if (queue_) {
            queue_->Close();
            queue_.reset();
        }
        if (sourceTimer_) {
            CloseHandle(sourceTimer_);
            sourceTimer_ = nullptr;
        }
        if (stopEvent_) {
            CloseHandle(stopEvent_);
            stopEvent_ = nullptr;
        }
        OmniphonySpatialProviderModuleRelease();
    }

    HRESULT OpenEndpoint(const wchar_t* physicalEndpointId) noexcept {
        stopEvent_ = CreateEventW(nullptr, TRUE, FALSE, nullptr);
        if (!stopEvent_) {
            return LastErrorOrFail();
        }
        return pump_.Open(physicalEndpointId, queue_);
    }

    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** object) override {
        if (!object) {
            return E_POINTER;
        }
        *object = nullptr;
        if (IsEqualIID(riid, IID_IUnknown) ||
            IsEqualIID(riid, __uuidof(ISpatialAudioObjectRenderStreamBase)) ||
            IsEqualIID(riid, __uuidof(ISpatialAudioObjectRenderStream))) {
            *object = static_cast<ISpatialAudioObjectRenderStream*>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }

    ULONG STDMETHODCALLTYPE AddRef() override {
        return references_.fetch_add(1, std::memory_order_relaxed) + 1;
    }

    ULONG STDMETHODCALLTYPE Release() override {
        const ULONG value = references_.fetch_sub(1, std::memory_order_acq_rel) - 1;
        if (value == 0) {
            delete this;
        }
        return value;
    }

    HRESULT STDMETHODCALLTYPE GetAvailableDynamicObjectCount(UINT32* count) override {
        const HRESULT async = AsyncResult();
        return FAILED(async) ? async : inner_->GetAvailableDynamicObjectCount(count);
    }

    HRESULT STDMETHODCALLTYPE GetService(REFIID riid, void** service) override {
        const HRESULT async = AsyncResult();
        return FAILED(async) ? async : inner_->GetService(riid, service);
    }

    HRESULT STDMETHODCALLTYPE Start() override {
        std::lock_guard<std::mutex> lock(lifecycleMutex_);
        if (running_) {
            return SPTLAUDCLNT_E_STREAM_NOT_STOPPED;
        }
        if (!inner_ || !queue_ || !queue_->IsOpen() || !pump_.IsOpen() ||
            !clientEvent_ || !stopEvent_) {
            return E_UNEXPECTED;
        }

        ResetEvent(stopEvent_);
        asyncResult_.store(S_OK, std::memory_order_release);
        queue_->Reset();

        HRESULT result = inner_->Start();
        if (FAILED(result)) {
            return result;
        }

        result = pump_.Start();
        if (FAILED(result)) {
            (void)inner_->Stop();
            return result;
        }

        sourceTimer_ = CreateWaitableTimerW(nullptr, FALSE, nullptr);
        if (!sourceTimer_) {
            result = LastErrorOrFail();
            (void)pump_.Stop();
            (void)inner_->Stop();
            return result;
        }

        LARGE_INTEGER due{};
        due.QuadPart = -100'000LL; // 10 ms in 100 ns units.
        if (!SetWaitableTimer(
                sourceTimer_,
                &due,
                kSourcePeriodMilliseconds,
                nullptr,
                nullptr,
                FALSE)) {
            result = LastErrorOrFail();
            CloseHandle(sourceTimer_);
            sourceTimer_ = nullptr;
            (void)pump_.Stop();
            (void)inner_->Stop();
            return result;
        }

        try {
            endpointWorker_ = std::thread([this]() { EndpointWorker(); });
            sourceWorker_ = std::thread([this]() { SourceWorker(); });
        }
        catch (const std::system_error&) {
            SetEvent(stopEvent_);
            JoinWorkers();
            CancelWaitableTimer(sourceTimer_);
            CloseHandle(sourceTimer_);
            sourceTimer_ = nullptr;
            (void)pump_.Stop();
            (void)inner_->Stop();
            return E_FAIL;
        }

        // Do not signal the borrowed application event synchronously from Start.
        // The periodic source timer owns every update notification. This keeps a
        // failed SetEvent in the worker's observable async-failure path and keeps
        // Start itself fully rollback-safe.
        running_ = true;
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE Stop() override {
        std::lock_guard<std::mutex> lock(lifecycleMutex_);
        if (!running_) {
            return S_OK;
        }

        SetEvent(stopEvent_);
        JoinWorkers();
        if (sourceTimer_) {
            CancelWaitableTimer(sourceTimer_);
            CloseHandle(sourceTimer_);
            sourceTimer_ = nullptr;
        }

        const HRESULT async = AsyncResult();
        const HRESULT pumpResult = pump_.Stop();
        const HRESULT innerResult = inner_->Stop();
        running_ = false;

        if (FAILED(async)) {
            return async;
        }
        if (FAILED(pumpResult)) {
            return pumpResult;
        }
        return innerResult;
    }

    HRESULT STDMETHODCALLTYPE Reset() override {
        std::lock_guard<std::mutex> lock(lifecycleMutex_);
        if (running_) {
            return SPTLAUDCLNT_E_STREAM_NOT_STOPPED;
        }
        const HRESULT result = inner_->Reset();
        if (FAILED(result)) {
            return result;
        }
        if (queue_) {
            queue_->Reset();
        }
        asyncResult_.store(S_OK, std::memory_order_release);
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE BeginUpdatingAudioObjects(
        UINT32* availableDynamicObjectCount,
        UINT32* frameCountPerBuffer) override {
        const HRESULT async = AsyncResult();
        if (FAILED(async)) {
            return async;
        }
        return inner_->BeginUpdatingAudioObjects(
            availableDynamicObjectCount,
            frameCountPerBuffer);
    }

    HRESULT STDMETHODCALLTYPE EndUpdatingAudioObjects() override {
        const HRESULT async = AsyncResult();
        if (FAILED(async)) {
            return async;
        }
        return inner_->EndUpdatingAudioObjects();
    }

    HRESULT STDMETHODCALLTYPE ActivateSpatialAudioObject(
        AudioObjectType type,
        ISpatialAudioObject** object) override {
        const HRESULT async = AsyncResult();
        if (FAILED(async)) {
            if (object) {
                *object = nullptr;
            }
            return async;
        }
        return inner_->ActivateSpatialAudioObject(type, object);
    }

private:
    HRESULT AsyncResult() const noexcept {
        return asyncResult_.load(std::memory_order_acquire);
    }

    void SetAsyncFailure(HRESULT result) noexcept {
        if (SUCCEEDED(result)) {
            return;
        }
        HRESULT expected = S_OK;
        if (asyncResult_.compare_exchange_strong(
                expected,
                result,
                std::memory_order_acq_rel,
                std::memory_order_acquire)) {
            if (stopEvent_) {
                SetEvent(stopEvent_);
            }
        }
    }

    void EndpointWorker() noexcept {
        DWORD taskIndex = 0;
        HANDLE mmcss = AvSetMmThreadCharacteristicsW(L"Pro Audio", &taskIndex);

        HANDLE handles[2] = {stopEvent_, pump_.SampleReadyEvent()};
        for (;;) {
            const DWORD waitResult = WaitForMultipleObjects(2, handles, FALSE, INFINITE);
            if (waitResult == WAIT_OBJECT_0) {
                break;
            }
            if (waitResult != WAIT_OBJECT_0 + 1) {
                SetAsyncFailure(
                    waitResult == WAIT_FAILED ? LastErrorOrFail() : E_UNEXPECTED);
                break;
            }

            const HRESULT drain = pump_.DrainOnce();
            if (FAILED(drain)) {
                SetAsyncFailure(drain);
                break;
            }
        }

        if (mmcss) {
            AvRevertMmThreadCharacteristics(mmcss);
        }
    }

    void SourceWorker() noexcept {
        HANDLE handles[2] = {stopEvent_, sourceTimer_};
        for (;;) {
            const DWORD waitResult = WaitForMultipleObjects(2, handles, FALSE, INFINITE);
            if (waitResult == WAIT_OBJECT_0) {
                break;
            }
            if (waitResult != WAIT_OBJECT_0 + 1) {
                SetAsyncFailure(
                    waitResult == WAIT_FAILED ? LastErrorOrFail() : E_UNEXPECTED);
                break;
            }
            if (!SetEvent(clientEvent_)) {
                SetAsyncFailure(LastErrorOrFail());
                break;
            }
        }
    }

    void JoinWorkers() noexcept {
        if (endpointWorker_.joinable()) {
            endpointWorker_.join();
        }
        if (sourceWorker_.joinable()) {
            sourceWorker_.join();
        }
    }

    std::atomic<ULONG> references_{1};
    ISpatialAudioObjectRenderStream* inner_ = nullptr;
    std::shared_ptr<OmniphonySpatialStereoQueue> queue_;
    HANDLE clientEvent_ = nullptr; // Borrowed from Windows activation params.
    HANDLE stopEvent_ = nullptr;
    HANDLE sourceTimer_ = nullptr;
    OmniphonySpatialRawOutputPump pump_;
    std::thread endpointWorker_;
    std::thread sourceWorker_;
    std::mutex lifecycleMutex_;
    std::atomic<HRESULT> asyncResult_{S_OK};
    bool running_ = false;
};

} // namespace

HRESULT CreateOmniphonySpatialProviderStaticStreamFromActivation(
    const PROPVARIANT* activationParams,
    REFIID riid,
    const wchar_t* realtimeDllPath,
    const wchar_t* physicalEndpointId,
    void** stream) {
    if (!stream) {
        return E_POINTER;
    }
    *stream = nullptr;
    if (!IsEqualIID(riid, __uuidof(ISpatialAudioObjectRenderStream))) {
        return E_NOINTERFACE;
    }
    if (!realtimeDllPath || !realtimeDllPath[0] ||
        !physicalEndpointId || !physicalEndpointId[0]) {
        return E_INVALIDARG;
    }

    SpatialAudioObjectRenderStreamActivationParams params{};
    if (!ValidStaticActivationBlob(activationParams, params)) {
        return E_INVALIDARG;
    }

    std::shared_ptr<OmniphonySpatialStereoQueue> queue;
    try {
        queue = std::make_shared<OmniphonySpatialStereoQueue>();
    }
    catch (const std::bad_alloc&) {
        return E_OUTOFMEMORY;
    }
    if (!queue->Open(kProviderQueueFrames)) {
        return E_OUTOFMEMORY;
    }

    ISpatialAudioObjectRenderStream* inner = nullptr;
    HRESULT result = CreateOmniphonyStaticProbeStreamWithRealtimeBridgeAndQueue(
        params,
        realtimeDllPath,
        queue,
        &inner);
    if (FAILED(result) || !inner) {
        queue->Close();
        return FAILED(result) ? result : E_FAIL;
    }

    ProviderStaticRenderStream* created = nullptr;
    try {
        created = new ProviderStaticRenderStream(inner, queue, params.EventHandle);
    }
    catch (const std::bad_alloc&) {
        inner->Release();
        queue->Close();
        return E_OUTOFMEMORY;
    }

    result = created->OpenEndpoint(physicalEndpointId);
    if (FAILED(result)) {
        created->Release();
        return result;
    }

    *stream = static_cast<ISpatialAudioObjectRenderStream*>(created);
    return S_OK;
}
