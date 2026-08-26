#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <cstring>
#include <memory>
#include <new>
#include <utility>
#include <vector>

#include "OmniphonySpatialObjectRealtimeBridge.h"
#include "OmniphonySpatialObjectStream.h"
#include "OmniphonySpatialRoles.h"
#include "OmniphonySpatialStereoQueue.h"

namespace {

bool IsAbsoluteWindowsPath(const wchar_t* path) noexcept {
    if (!path || !path[0]) {
        return false;
    }
    if (path[0] == L'\\' && path[1] == L'\\') {
        return true;
    }
    const wchar_t drive = path[0];
    const bool asciiLetter =
        (drive >= L'A' && drive <= L'Z') || (drive >= L'a' && drive <= L'z');
    return asciiLetter &&
           path[1] == L':' &&
           (path[2] == L'\\' || path[2] == L'/');
}

template <typename T>
bool Resolve(HMODULE module, const char* name, T& target) noexcept {
    const FARPROC raw = GetProcAddress(module, name);
    if (!raw) {
        target = nullptr;
        return false;
    }
    static_assert(sizeof(raw) == sizeof(target));
    std::memcpy(&target, &raw, sizeof(target));
    return target != nullptr;
}

HRESULT LastErrorOrFail() noexcept {
    const DWORD error = GetLastError();
    return error == ERROR_SUCCESS ? E_FAIL : HRESULT_FROM_WIN32(error);
}

class ObjectRealtimeBridge final {
public:
    ~ObjectRealtimeBridge() { Close(); }

    HRESULT Open(
        const wchar_t* realtimeDllPath,
        const SpatialAudioObjectRenderStreamActivationParams& params) noexcept {
        Close();
        if (!IsAbsoluteWindowsPath(realtimeDllPath) || !params.ObjectFormat) {
            return E_INVALIDARG;
        }

        try {
            descriptors_.reserve(OmniphonyStaticRoleCount(params.StaticObjectTypeMask));
            for (const auto& role : kOmniphonySpatialStaticRoles) {
                if ((OmniphonySpatialObjectBits(params.StaticObjectTypeMask) &
                     OmniphonySpatialObjectBits(role.audio_object_type)) == 0) {
                    continue;
                }
                descriptors_.push_back({
                    role.omniphony_role,
                    role.x_right_m,
                    role.y_up_m,
                    role.z_back_m,
                });
            }
        }
        catch (const std::bad_alloc&) {
            return E_OUTOFMEMORY;
        }

        if (descriptors_.empty() && params.MaxDynamicObjectCount == 0) {
            return AUDCLNT_E_UNSUPPORTED_FORMAT;
        }

        module_ = LoadLibraryExW(
            realtimeDllPath,
            nullptr,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
        if (!module_) {
            return LastErrorOrFail();
        }

        AbiVersionFn abiMajor = nullptr;
        AbiVersionFn abiMinor = nullptr;
        CreateFn create = nullptr;
        if (!Resolve(module_, "omniphony_realtime_abi_major", abiMajor) ||
            !Resolve(module_, "omniphony_realtime_abi_minor", abiMinor) ||
            !Resolve(module_, "omniphony_spatial_objects_create", create) ||
            !Resolve(module_, "omniphony_spatial_objects_destroy", destroy_) ||
            !Resolve(module_, "omniphony_spatial_objects_latency_frames", latency_) ||
            !Resolve(module_, "omniphony_spatial_objects_processed_blocks", processedBlocks_) ||
            !Resolve(module_, "omniphony_spatial_objects_process_f32", process_) ||
            !Resolve(module_, "omniphony_spatial_objects_reset", reset_)) {
            Close();
            return HRESULT_FROM_WIN32(ERROR_PROC_NOT_FOUND);
        }

        if (abiMajor() != OMNIPHONY_REALTIME_ABI_MAJOR ||
            abiMinor() < OMNIPHONY_REALTIME_ABI_MINOR) {
            Close();
            return HRESULT_FROM_WIN32(ERROR_REVISION_MISMATCH);
        }

        OmniphonySpatialObjectConfig config{};
        config.sample_rate_hz = params.ObjectFormat->nSamplesPerSec;
        config.frames_per_quantum = 480;
        config.static_object_count = static_cast<std::uint32_t>(descriptors_.size());
        config.static_objects = descriptors_.empty() ? nullptr : descriptors_.data();
        config.max_dynamic_objects = params.MaxDynamicObjectCount;

        processor_ = create(&config);
        if (!processor_) {
            Close();
            return E_FAIL;
        }
        return S_OK;
    }

    void Close() noexcept {
        if (processor_ && destroy_) {
            destroy_(processor_);
        }
        processor_ = nullptr;
        destroy_ = nullptr;
        latency_ = nullptr;
        processedBlocks_ = nullptr;
        process_ = nullptr;
        reset_ = nullptr;
        descriptors_.clear();

        if (module_) {
            FreeLibrary(module_);
            module_ = nullptr;
        }
    }

    HRESULT Process(
        const float* staticInputPlanar,
        const OmniphonySpatialDynamicObjectDescriptor* dynamicObjects,
        std::uint32_t dynamicObjectCount,
        const float* dynamicInputPlanar,
        float* outputStereo,
        std::size_t frames) noexcept {
        if (!processor_ || !process_) {
            return E_UNEXPECTED;
        }
        if (!outputStereo || frames == 0) {
            return E_INVALIDARG;
        }
        if (!descriptors_.empty() && !staticInputPlanar) {
            return E_INVALIDARG;
        }
        if (dynamicObjectCount > 0 && (!dynamicObjects || !dynamicInputPlanar)) {
            return E_INVALIDARG;
        }

        const std::int32_t result = process_(
            processor_,
            staticInputPlanar,
            dynamicObjects,
            dynamicObjectCount,
            dynamicInputPlanar,
            outputStereo,
            frames);
        return result == 0 ? S_OK : HRESULT_FROM_WIN32(ERROR_INVALID_DATA);
    }

    HRESULT Reset() noexcept {
        if (!processor_ || !reset_) {
            return E_UNEXPECTED;
        }
        const std::int32_t result = reset_(processor_);
        return result == 0 ? S_OK : HRESULT_FROM_WIN32(ERROR_INVALID_STATE);
    }

private:
    using AbiVersionFn = std::uint32_t (*)();
    using CreateFn = OmniphonySpatialObjectProcessor* (*)(
        const OmniphonySpatialObjectConfig*);
    using DestroyFn = void (*)(OmniphonySpatialObjectProcessor*);
    using LatencyFn = std::size_t (*)(const OmniphonySpatialObjectProcessor*);
    using ProcessedBlocksFn = std::uint64_t (*)(const OmniphonySpatialObjectProcessor*);
    using ProcessFn = std::int32_t (*)(
        OmniphonySpatialObjectProcessor*,
        const float*,
        const OmniphonySpatialDynamicObjectDescriptor*,
        std::uint32_t,
        const float*,
        float*,
        std::size_t);

    using ResetFn = std::int32_t (*)(OmniphonySpatialObjectProcessor*);

    HMODULE module_ = nullptr;
    OmniphonySpatialObjectProcessor* processor_ = nullptr;
    DestroyFn destroy_ = nullptr;
    LatencyFn latency_ = nullptr;
    ProcessedBlocksFn processedBlocks_ = nullptr;
    ProcessFn process_ = nullptr;
    ResetFn reset_ = nullptr;
    std::vector<OmniphonySpatialStaticObjectDescriptor> descriptors_;
};

class RealtimeObjectQuantumTransport final : public OmniphonySpatialObjectQuantumTransport {
public:
    HRESULT Reset() noexcept override {
        if (!stereoQueue_) {
            return E_UNEXPECTED;
        }
        const HRESULT result = bridge_.Reset();
        if (FAILED(result)) {
            return result;
        }
        stereoQueue_->Reset();
        return S_OK;
    }

    HRESULT Open(
        const wchar_t* realtimeDllPath,
        const SpatialAudioObjectRenderStreamActivationParams& params,
        std::shared_ptr<OmniphonySpatialStereoQueue> stereoQueue) {
        if (!stereoQueue || !stereoQueue->IsOpen()) {
            return E_INVALIDARG;
        }
        const HRESULT result = bridge_.Open(realtimeDllPath, params);
        if (FAILED(result)) {
            return result;
        }
        stereoQueue_ = std::move(stereoQueue);
        return S_OK;
    }

    HRESULT Process(
        const float* staticInputPlanar,
        const OmniphonySpatialDynamicObjectDescriptor* dynamicObjects,
        std::uint32_t dynamicObjectCount,
        const float* dynamicInputPlanar,
        float* outputStereo,
        std::size_t frames) noexcept override {
        const HRESULT result = bridge_.Process(
            staticInputPlanar,
            dynamicObjects,
            dynamicObjectCount,
            dynamicInputPlanar,
            outputStereo,
            frames);
        if (FAILED(result)) {
            return result;
        }
        if (!stereoQueue_->TryWrite(outputStereo, frames)) {
            return HRESULT_FROM_WIN32(ERROR_BUFFER_OVERFLOW);
        }
        return S_OK;
    }

private:
    ObjectRealtimeBridge bridge_;
    std::shared_ptr<OmniphonySpatialStereoQueue> stereoQueue_;
};

} // namespace

HRESULT CreateOmniphonySpatialObjectStreamWithRealtimeBridgeAndQueue(
    const SpatialAudioObjectRenderStreamActivationParams& params,
    const wchar_t* realtimeDllPath,
    std::shared_ptr<OmniphonySpatialStereoQueue> stereoQueue,
    ISpatialAudioObjectRenderStream** stream) {
    if (!stream) {
        return E_POINTER;
    }
    *stream = nullptr;
    if (!IsAbsoluteWindowsPath(realtimeDllPath) || !stereoQueue || !stereoQueue->IsOpen()) {
        return E_INVALIDARG;
    }

    std::shared_ptr<RealtimeObjectQuantumTransport> transport;
    try {
        transport = std::make_shared<RealtimeObjectQuantumTransport>();
    }
    catch (const std::bad_alloc&) {
        return E_OUTOFMEMORY;
    }

    const HRESULT openResult = transport->Open(
        realtimeDllPath,
        params,
        std::move(stereoQueue));
    if (FAILED(openResult)) {
        return openResult;
    }

    return CreateOmniphonySpatialObjectStreamWithTransport(
        params,
        std::move(transport),
        stream);
}
