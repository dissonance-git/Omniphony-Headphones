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

#include "OmniphonySpatialObjectRealtimeBridge.h"
#include "OmniphonySpatialProviderRuntime.h"
#include "OmniphonySpatialRawOutputPump.h"
#include "OmniphonySpatialStereoQueue.h"

namespace {

constexpr std::size_t kSourceQuantumFrames = 480u;
constexpr std::size_t kProviderQueueFrames = kSourceQuantumFrames * 8u;
constexpr std::size_t kProviderTargetQueuedFrames = kSourceQuantumFrames * 4u;
constexpr UINT32 kProviderMaxDynamicObjects = 16;

HRESULT LastErrorOrFail() noexcept {
    const DWORD error = GetLastError();
    return HRESULT_FROM_WIN32(error == ERROR_SUCCESS ? ERROR_GEN_FAILURE : error);
}

bool ValidActivationBlob(
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
           params.MinDynamicObjectCount <= params.MaxDynamicObjectCount &&
           params.MaxDynamicObjectCount <= kProviderMaxDynamicObjects;
}

class ProviderObjectRenderStream final : public ISpatialAudioObjectRenderStream {
public:
    ProviderObjectRenderStream(
        ISpatialAudioObjectRenderStream* inner,
        std::shared_ptr<OmniphonySpatialStereoQueue> queue,
        HANDLE clientEvent) noexcept
        : inner_(inner),
          queue_(std::move(queue)),
          clientEvent_(clientEvent) {
        OmniphonySpatialProviderModuleAddRef();
    }

    ~ProviderObjectRenderStream() {
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

        sourceRequestOutstanding_.store(false, std::memory_order_release);
        updateInProgress_.store(false, std::memory_order_release);

        try {
            endpointWorker_ = std::thread([this]() { EndpointWorker(); });
        }
        catch (const std::system_error&) {
            SetEvent(stopEvent_);
            JoinWorkers();
            (void)pump_.Stop();
            (void)inner_->Stop();
            return E_FAIL;
        }

        running_ = true;

        // Prime one source request immediately. Subsequent requests are driven
        // by queue demand and the physical endpoint's actual drain cadence.
        result = RequestSourceIfNeeded();
        if (FAILED(result)) {
            SetEvent(stopEvent_);
            JoinWorkers();
            (void)pump_.Stop();
            (void)inner_->Stop();
            running_ = false;
            return result;
        }
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE Stop() override {
        std::lock_guard<std::mutex> lock(lifecycleMutex_);
        if (!running_) {
            return S_OK;
        }

        SetEvent(stopEvent_);
        JoinWorkers();
        sourceRequestOutstanding_.store(false, std::memory_order_release);
        updateInProgress_.store(false, std::memory_order_release);
        ResetEvent(clientEvent_);

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
        sourceRequestOutstanding_.store(false, std::memory_order_release);
        updateInProgress_.store(false, std::memory_order_release);
        ResetEvent(clientEvent_);
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
        const HRESULT result = inner_->BeginUpdatingAudioObjects(
            availableDynamicObjectCount,
            frameCountPerBuffer);
        if (SUCCEEDED(result)) {
            sourceRequestOutstanding_.store(false, std::memory_order_release);
            updateInProgress_.store(true, std::memory_order_release);
        }
        return result;
    }

    HRESULT STDMETHODCALLTYPE EndUpdatingAudioObjects() override {
        const HRESULT async = AsyncResult();
        if (FAILED(async)) {
            return async;
        }
        const HRESULT result = inner_->EndUpdatingAudioObjects();
        updateInProgress_.store(false, std::memory_order_release);
        if (FAILED(result)) {
            return result;
        }
        return RequestSourceIfNeeded();
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
    HRESULT RequestSourceIfNeeded() noexcept {
        if (!clientEvent_ || !queue_ || !queue_->IsOpen()) {
            return E_UNEXPECTED;
        }
        if (updateInProgress_.load(std::memory_order_acquire)) {
            return S_OK;
        }

        // Keep a bounded producer lead, then let physical endpoint consumption
        // create demand. This removes the former free-running 10 ms source
        // timer and keeps one clock owner downstream.
        const std::size_t queued = queue_->AvailableFrames();
        if (queued + kSourceQuantumFrames > kProviderTargetQueuedFrames) {
            return S_OK;
        }

        bool expected = false;
        if (!sourceRequestOutstanding_.compare_exchange_strong(
                expected,
                true,
                std::memory_order_acq_rel,
                std::memory_order_acquire)) {
            return S_OK;
        }

        if (!SetEvent(clientEvent_)) {
            sourceRequestOutstanding_.store(false, std::memory_order_release);
            return LastErrorOrFail();
        }
        return S_OK;
    }

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

            const HRESULT sourceDemand = RequestSourceIfNeeded();
            if (FAILED(sourceDemand)) {
                SetAsyncFailure(sourceDemand);
                break;
            }
        }

        if (mmcss) {
            AvRevertMmThreadCharacteristics(mmcss);
        }
    }

    void JoinWorkers() noexcept {
        if (endpointWorker_.joinable()) {
            endpointWorker_.join();
        }
    }

    std::atomic<ULONG> references_{1};
    ISpatialAudioObjectRenderStream* inner_ = nullptr;
    std::shared_ptr<OmniphonySpatialStereoQueue> queue_;
    HANDLE clientEvent_ = nullptr;
    HANDLE stopEvent_ = nullptr;
    OmniphonySpatialRawOutputPump pump_;
    std::thread endpointWorker_;
    std::mutex lifecycleMutex_;
    std::atomic<HRESULT> asyncResult_{S_OK};
    std::atomic<bool> sourceRequestOutstanding_{false};
    std::atomic<bool> updateInProgress_{false};
    bool running_ = false;
};

} // namespace

HRESULT CreateOmniphonySpatialProviderObjectStreamFromActivation(
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
    if (!ValidActivationBlob(activationParams, params)) {
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
    HRESULT result = CreateOmniphonySpatialObjectStreamWithRealtimeBridgeAndQueue(
        params,
        realtimeDllPath,
        queue,
        &inner);
    if (FAILED(result) || !inner) {
        queue->Close();
        return FAILED(result) ? result : E_FAIL;
    }

    ProviderObjectRenderStream* created = nullptr;
    try {
        created = new ProviderObjectRenderStream(inner, queue, params.EventHandle);
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
