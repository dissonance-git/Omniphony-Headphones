#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <mmreg.h>
#include <spatialaudioclient.h>

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdint>
#include <memory>
#include <mutex>
#include <new>
#include <vector>

#include "OmniphonySpatialObjectStream.h"
#include "OmniphonySpatialRoles.h"

namespace {

constexpr UINT32 kFramesPerQuantum = 480;
constexpr UINT32 kProviderMaxDynamicObjects = 16;
constexpr std::uint64_t kDynamicIdBase = 0x4459'4E41'0000'0001ull; // "DYNA"

struct ObjectStreamState;
class ProviderSpatialObject;

struct ObjectStreamState {
    std::mutex mutex;
    bool destroyed = false;
    bool running = false;
    bool inUpdate = false;
    bool transportInFlight = false;
    std::uint64_t generation = 0;
    AudioObjectType staticMask = AudioObjectType_None;
    UINT32 frameCount = kFramesPerQuantum;
    UINT32 maxDynamicObjects = 0;
    std::uint64_t nextDynamicId = kDynamicIdBase;
    std::vector<ProviderSpatialObject*> objects;
};

class ProviderSpatialObject final : public ISpatialAudioObject {
public:
    ProviderSpatialObject(
        std::shared_ptr<ObjectStreamState> state,
        AudioObjectType type,
        UINT32 frameCount,
        std::uint64_t stableId)
        : state_(std::move(state)),
          type_(type),
          stableId_(stableId),
          staging_(frameCount, 0.0f) {
        std::lock_guard<std::mutex> lock(state_->mutex);
        state_->objects.push_back(this);
    }

    ~ProviderSpatialObject() {
        std::lock_guard<std::mutex> lock(state_->mutex);
        auto& objects = state_->objects;
        objects.erase(std::remove(objects.begin(), objects.end(), this), objects.end());
    }

    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID riid, void** object) override {
        if (!object) {
            return E_POINTER;
        }
        *object = nullptr;
        if (IsEqualIID(riid, IID_IUnknown) ||
            IsEqualIID(riid, __uuidof(ISpatialAudioObjectBase)) ||
            IsEqualIID(riid, __uuidof(ISpatialAudioObject))) {
            *object = static_cast<ISpatialAudioObject*>(this);
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

    HRESULT STDMETHODCALLTYPE GetBuffer(BYTE** buffer, UINT32* bufferLength) override {
        if (!buffer || !bufferLength) {
            return E_POINTER;
        }
        *buffer = nullptr;
        *bufferLength = 0;

        std::lock_guard<std::mutex> stateLock(state_->mutex);
        if (state_->destroyed) {
            return SPTLAUDCLNT_E_DESTROYED;
        }
        if (!state_->inUpdate || state_->transportInFlight) {
            return SPTLAUDCLNT_E_OUT_OF_ORDER;
        }

        std::lock_guard<std::mutex> objectLock(mutex_);
        if (!active_) {
            return SPTLAUDCLNT_E_RESOURCES_INVALIDATED;
        }
        std::fill(staging_.begin(), staging_.end(), 0.0f);
        lastBufferGeneration_ = state_->generation;
        endOfStreamPending_ = false;
        endOfStreamFrameCount_ = 0;
        *buffer = reinterpret_cast<BYTE*>(staging_.data());
        *bufferLength = static_cast<UINT32>(staging_.size() * sizeof(float));
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE SetEndOfStream(UINT32 frameCount) override {
        std::lock_guard<std::mutex> stateLock(state_->mutex);
        if (state_->destroyed) {
            return SPTLAUDCLNT_E_DESTROYED;
        }
        if (!state_->inUpdate || state_->transportInFlight) {
            return SPTLAUDCLNT_E_OUT_OF_ORDER;
        }
        if (frameCount > state_->frameCount) {
            return E_INVALIDARG;
        }

        std::lock_guard<std::mutex> objectLock(mutex_);
        if (!active_) {
            return SPTLAUDCLNT_E_RESOURCES_INVALIDATED;
        }
        endOfStreamPending_ = true;
        endOfStreamFrameCount_ = frameCount;
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE IsActive(BOOL* isActive) override {
        if (!isActive) {
            return E_POINTER;
        }
        std::lock_guard<std::mutex> stateLock(state_->mutex);
        if (state_->destroyed) {
            *isActive = FALSE;
            return SPTLAUDCLNT_E_DESTROYED;
        }
        std::lock_guard<std::mutex> objectLock(mutex_);
        *isActive = active_ ? TRUE : FALSE;
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE GetAudioObjectType(AudioObjectType* type) override {
        if (!type) {
            return E_POINTER;
        }
        *type = type_;
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE SetPosition(float x, float y, float z) override {
        if (!std::isfinite(x) || !std::isfinite(y) || !std::isfinite(z)) {
            return E_INVALIDARG;
        }
        std::lock_guard<std::mutex> stateLock(state_->mutex);
        if (state_->destroyed) {
            return SPTLAUDCLNT_E_DESTROYED;
        }
        if (!state_->inUpdate || state_->transportInFlight) {
            return SPTLAUDCLNT_E_OUT_OF_ORDER;
        }
        std::lock_guard<std::mutex> objectLock(mutex_);
        if (!active_) {
            return SPTLAUDCLNT_E_RESOURCES_INVALIDATED;
        }
        if (type_ != AudioObjectType_Dynamic) {
            return SPTLAUDCLNT_E_PROPERTY_NOT_SUPPORTED;
        }
        // Windows defines origin as the dynamic-object default and the last set
        // position persists until another SetPosition call.
        xRightM_ = x;
        yUpM_ = y;
        zBackM_ = z;
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE SetVolume(float volume) override {
        if (!std::isfinite(volume) || volume < 0.0f || volume > 1.0f) {
            return E_INVALIDARG;
        }
        std::lock_guard<std::mutex> stateLock(state_->mutex);
        if (state_->destroyed) {
            return SPTLAUDCLNT_E_DESTROYED;
        }
        if (!state_->inUpdate || state_->transportInFlight) {
            return SPTLAUDCLNT_E_OUT_OF_ORDER;
        }
        std::lock_guard<std::mutex> objectLock(mutex_);
        if (!active_) {
            return SPTLAUDCLNT_E_RESOURCES_INVALIDATED;
        }
        volume_ = volume;
        return S_OK;
    }

    void SnapshotStatic(
        std::uint64_t generation,
        float* destination,
        UINT32 frameCount) noexcept {
        std::lock_guard<std::mutex> lock(mutex_);
        if (!active_) {
            return;
        }
        if (lastBufferGeneration_ != generation) {
            active_ = false;
            return;
        }
        CopyFinalPass(destination, frameCount);
    }

    bool SnapshotDynamic(
        std::uint64_t generation,
        OmniphonySpatialDynamicObjectDescriptor& descriptor,
        float* destination,
        UINT32 frameCount) noexcept {
        std::lock_guard<std::mutex> lock(mutex_);
        if (!active_) {
            return false;
        }
        if (lastBufferGeneration_ != generation) {
            active_ = false;
            return false;
        }

        descriptor.stable_id = stableId_;
        descriptor.x_right_m = xRightM_;
        descriptor.y_up_m = yUpM_;
        descriptor.z_back_m = zBackM_;
        CopyFinalPass(destination, frameCount);
        return true;
    }

    void Revoke() noexcept {
        std::lock_guard<std::mutex> lock(mutex_);
        active_ = false;
    }

    bool IsActiveInternal() const noexcept {
        std::lock_guard<std::mutex> lock(mutex_);
        return active_;
    }

    AudioObjectType Type() const noexcept { return type_; }

private:
    void CopyFinalPass(float* destination, UINT32 frameCount) noexcept {
        UINT32 validFrames = frameCount;
        if (endOfStreamPending_) {
            validFrames = std::min(frameCount, endOfStreamFrameCount_);
        }
        validFrames = std::min(validFrames, static_cast<UINT32>(staging_.size()));
        for (UINT32 frame = 0; frame < validFrames; ++frame) {
            destination[frame] = staging_[frame] * volume_;
        }
        if (endOfStreamPending_) {
            active_ = false;
        }
    }

    std::atomic<ULONG> references_{1};
    std::shared_ptr<ObjectStreamState> state_;
    AudioObjectType type_ = AudioObjectType_None;
    std::uint64_t stableId_ = 0;
    mutable std::mutex mutex_;
    std::vector<float> staging_;
    bool active_ = true;
    std::uint64_t lastBufferGeneration_ = 0;
    bool endOfStreamPending_ = false;
    UINT32 endOfStreamFrameCount_ = 0;
    float volume_ = 1.0f;
    float xRightM_ = 0.0f;
    float yUpM_ = 0.0f;
    float zBackM_ = 0.0f;
};

UINT32 LiveDynamicObjectCount(const ObjectStreamState& state) noexcept {
    UINT32 count = 0;
    for (const auto* object : state.objects) {
        if (object && object->Type() == AudioObjectType_Dynamic) {
            ++count;
        }
    }
    return count;
}

class ProviderObjectStream final : public ISpatialAudioObjectRenderStream {
public:
    ProviderObjectStream(
        AudioObjectType staticMask,
        UINT32 frameCount,
        UINT32 maxDynamicObjects,
        std::shared_ptr<OmniphonySpatialObjectQuantumTransport> transport)
        : state_(std::make_shared<ObjectStreamState>()),
          transport_(std::move(transport)) {
        state_->staticMask = staticMask;
        state_->frameCount = frameCount;
        state_->maxDynamicObjects = maxDynamicObjects;

        const auto roleCount = OmniphonyStaticRoleCount(staticMask);
        roleOrder_.reserve(roleCount);
        for (const auto& role : kOmniphonySpatialStaticRoles) {
            if ((OmniphonySpatialObjectBits(staticMask) &
                 OmniphonySpatialObjectBits(role.audio_object_type)) != 0) {
                roleOrder_.push_back(role.audio_object_type);
            }
        }

        staticPlanar_.assign(roleCount * static_cast<std::size_t>(frameCount), 0.0f);
        dynamicDescriptors_.resize(maxDynamicObjects);
        dynamicPlanar_.assign(
            static_cast<std::size_t>(maxDynamicObjects) * frameCount,
            0.0f);
        stereo_.assign(static_cast<std::size_t>(frameCount) * 2, 0.0f);
    }

    ~ProviderObjectStream() {
        std::lock_guard<std::mutex> lock(state_->mutex);
        state_->destroyed = true;
        state_->running = false;
        state_->inUpdate = false;
        state_->transportInFlight = false;
        for (auto* object : state_->objects) {
            if (object) {
                object->Revoke();
            }
        }
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
        if (!count) {
            return E_POINTER;
        }
        std::lock_guard<std::mutex> lock(state_->mutex);
        if (state_->destroyed) {
            return SPTLAUDCLNT_E_DESTROYED;
        }
        const UINT32 live = LiveDynamicObjectCount(*state_);
        *count = live >= state_->maxDynamicObjects
            ? 0
            : state_->maxDynamicObjects - live;
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE GetService(REFIID, void** service) override {
        if (!service) {
            return E_POINTER;
        }
        *service = nullptr;
        return E_NOINTERFACE;
    }

    HRESULT STDMETHODCALLTYPE Start() override {
        std::lock_guard<std::mutex> lock(state_->mutex);
        if (state_->destroyed) {
            return SPTLAUDCLNT_E_DESTROYED;
        }
        if (state_->running) {
            return SPTLAUDCLNT_E_STREAM_NOT_STOPPED;
        }
        if (state_->inUpdate || state_->transportInFlight) {
            return SPTLAUDCLNT_E_OUT_OF_ORDER;
        }
        state_->running = true;
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE Stop() override {
        std::lock_guard<std::mutex> lock(state_->mutex);
        if (state_->destroyed) {
            return SPTLAUDCLNT_E_DESTROYED;
        }
        if (state_->inUpdate || state_->transportInFlight) {
            return SPTLAUDCLNT_E_OUT_OF_ORDER;
        }
        state_->running = false;
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE Reset() override {
        std::lock_guard<std::mutex> lock(state_->mutex);
        if (state_->destroyed) {
            return SPTLAUDCLNT_E_DESTROYED;
        }
        if (state_->running) {
            return SPTLAUDCLNT_E_STREAM_NOT_STOPPED;
        }
        if (state_->inUpdate || state_->transportInFlight) {
            return SPTLAUDCLNT_E_OUT_OF_ORDER;
        }
        state_->generation = 0;
        std::fill(staticPlanar_.begin(), staticPlanar_.end(), 0.0f);
        std::fill(dynamicPlanar_.begin(), dynamicPlanar_.end(), 0.0f);
        std::fill(stereo_.begin(), stereo_.end(), 0.0f);
        for (auto* object : state_->objects) {
            if (object) {
                object->Revoke();
            }
        }
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE BeginUpdatingAudioObjects(
        UINT32* availableDynamicObjectCount,
        UINT32* frameCountPerBuffer) override {
        if (!availableDynamicObjectCount || !frameCountPerBuffer) {
            return E_POINTER;
        }
        std::lock_guard<std::mutex> lock(state_->mutex);
        if (state_->destroyed) {
            return SPTLAUDCLNT_E_DESTROYED;
        }
        if (!state_->running) {
            return AUDCLNT_E_SERVICE_NOT_RUNNING;
        }
        if (state_->inUpdate || state_->transportInFlight) {
            return SPTLAUDCLNT_E_OUT_OF_ORDER;
        }
        state_->inUpdate = true;
        ++state_->generation;
        const UINT32 live = LiveDynamicObjectCount(*state_);
        *availableDynamicObjectCount = live >= state_->maxDynamicObjects
            ? 0
            : state_->maxDynamicObjects - live;
        *frameCountPerBuffer = state_->frameCount;
        return S_OK;
    }

    HRESULT STDMETHODCALLTYPE EndUpdatingAudioObjects() override {
        std::shared_ptr<OmniphonySpatialObjectQuantumTransport> transport;
        UINT32 frameCount = 0;
        UINT32 dynamicCount = 0;

        {
            std::lock_guard<std::mutex> lock(state_->mutex);
            if (state_->destroyed) {
                return SPTLAUDCLNT_E_DESTROYED;
            }
            if (!state_->inUpdate || state_->transportInFlight) {
                return SPTLAUDCLNT_E_OUT_OF_ORDER;
            }

            const auto generation = state_->generation;
            frameCount = state_->frameCount;
            std::fill(staticPlanar_.begin(), staticPlanar_.end(), 0.0f);
            std::fill(dynamicPlanar_.begin(), dynamicPlanar_.end(), 0.0f);

            for (auto* object : state_->objects) {
                if (!object) {
                    continue;
                }
                if (object->Type() == AudioObjectType_Dynamic) {
                    if (dynamicCount >= state_->maxDynamicObjects) {
                        continue;
                    }
                    if (object->SnapshotDynamic(
                            generation,
                            dynamicDescriptors_[dynamicCount],
                            dynamicPlanar_.data() +
                                static_cast<std::size_t>(dynamicCount) * frameCount,
                            frameCount)) {
                        ++dynamicCount;
                    }
                    continue;
                }

                const auto slot = OmniphonyStaticRoleSlot(
                    state_->staticMask,
                    object->Type());
                if (slot == static_cast<std::size_t>(-1) || slot >= roleOrder_.size()) {
                    continue;
                }
                object->SnapshotStatic(
                    generation,
                    staticPlanar_.data() + slot * frameCount,
                    frameCount);
            }

            state_->inUpdate = false;
            transport = transport_;
            if (transport) {
                state_->transportInFlight = true;
            }
        }

        if (!transport) {
            return S_OK;
        }

        std::fill(stereo_.begin(), stereo_.end(), 0.0f);
        const float* staticInput = staticPlanar_.empty() ? nullptr : staticPlanar_.data();
        const auto* dynamicInput = dynamicCount == 0 ? nullptr : dynamicDescriptors_.data();
        const float* dynamicPcm = dynamicCount == 0 ? nullptr : dynamicPlanar_.data();
        const HRESULT transportResult = transport->Process(
            staticInput,
            dynamicInput,
            dynamicCount,
            dynamicPcm,
            stereo_.data(),
            frameCount);

        {
            std::lock_guard<std::mutex> lock(state_->mutex);
            state_->transportInFlight = false;
        }
        return transportResult;
    }

    HRESULT STDMETHODCALLTYPE ActivateSpatialAudioObject(
        AudioObjectType type,
        ISpatialAudioObject** object) override {
        if (!object) {
            return E_POINTER;
        }
        *object = nullptr;

        std::unique_lock<std::mutex> lock(state_->mutex);
        if (state_->destroyed) {
            return SPTLAUDCLNT_E_DESTROYED;
        }
        if (!state_->inUpdate || state_->transportInFlight) {
            return SPTLAUDCLNT_E_OUT_OF_ORDER;
        }

        std::uint64_t stableId = 0;
        if (type == AudioObjectType_Dynamic) {
            if (LiveDynamicObjectCount(*state_) >= state_->maxDynamicObjects) {
                return SPTLAUDCLNT_E_NO_MORE_OBJECTS;
            }
            if (state_->nextDynamicId == 0) {
                return SPTLAUDCLNT_E_NO_MORE_OBJECTS;
            }
            stableId = state_->nextDynamicId++;
        } else {
            if (!OmniphonyIsSingleStaticObjectType(type) ||
                !FindOmniphonySpatialStaticRole(type)) {
                return E_INVALIDARG;
            }
            if ((OmniphonySpatialObjectBits(state_->staticMask) &
                 OmniphonySpatialObjectBits(type)) != OmniphonySpatialObjectBits(type)) {
                return SPTLAUDCLNT_E_STATIC_OBJECT_NOT_AVAILABLE;
            }
            for (auto* existing : state_->objects) {
                if (existing && existing->Type() == type && existing->IsActiveInternal()) {
                    return SPTLAUDCLNT_E_OBJECT_ALREADY_ACTIVE;
                }
            }
        }

        const UINT32 frameCount = state_->frameCount;
        lock.unlock();
        try {
            auto* created = new ProviderSpatialObject(
                state_, type, frameCount, stableId);
            *object = static_cast<ISpatialAudioObject*>(created);
            return S_OK;
        }
        catch (const std::bad_alloc&) {
            return E_OUTOFMEMORY;
        }
    }

private:
    std::atomic<ULONG> references_{1};
    std::shared_ptr<ObjectStreamState> state_;
    std::shared_ptr<OmniphonySpatialObjectQuantumTransport> transport_;
    std::vector<AudioObjectType> roleOrder_;
    std::vector<float> staticPlanar_;
    std::vector<OmniphonySpatialDynamicObjectDescriptor> dynamicDescriptors_;
    std::vector<float> dynamicPlanar_;
    std::vector<float> stereo_;
};

bool ValidActivationParams(const SpatialAudioObjectRenderStreamActivationParams& params) noexcept {
    const auto* format = params.ObjectFormat;
    if (!format ||
        format->wFormatTag != WAVE_FORMAT_IEEE_FLOAT ||
        format->nChannels != 1 ||
        format->nSamplesPerSec != 48'000 ||
        format->wBitsPerSample != 32 ||
        format->nBlockAlign != sizeof(float) ||
        format->nAvgBytesPerSec != 48'000 * sizeof(float)) {
        return false;
    }

    if (params.MinDynamicObjectCount > params.MaxDynamicObjectCount ||
        params.MaxDynamicObjectCount > kProviderMaxDynamicObjects) {
        return false;
    }

    const auto requestedMask = OmniphonySpatialObjectBits(params.StaticObjectTypeMask);
    const auto supportedMask = OmniphonySpatialObjectBits(OmniphonyCanonicalStaticMask());
    if ((requestedMask & ~supportedMask) != 0) {
        return false;
    }
    return requestedMask != 0 || params.MaxDynamicObjectCount != 0;
}

} // namespace

HRESULT CreateOmniphonySpatialObjectStreamWithTransport(
    const SpatialAudioObjectRenderStreamActivationParams& params,
    std::shared_ptr<OmniphonySpatialObjectQuantumTransport> transport,
    ISpatialAudioObjectRenderStream** stream) {
    if (!stream) {
        return E_POINTER;
    }
    *stream = nullptr;
    if (!transport) {
        return E_INVALIDARG;
    }
    if (!ValidActivationParams(params)) {
        return AUDCLNT_E_UNSUPPORTED_FORMAT;
    }

    try {
        auto* created = new ProviderObjectStream(
            params.StaticObjectTypeMask,
            kFramesPerQuantum,
            params.MaxDynamicObjectCount,
            std::move(transport));
        *stream = static_cast<ISpatialAudioObjectRenderStream*>(created);
        return S_OK;
    }
    catch (const std::bad_alloc&) {
        return E_OUTOFMEMORY;
    }
}
