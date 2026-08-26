#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>

#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <iostream>

#include "omniphony_realtime.h"

namespace {

using AbiFn = std::uint32_t (*)();
using CreateFn = OmniphonySpatialObjectProcessor* (*)(const OmniphonySpatialObjectConfig*);
using DestroyFn = void (*)(OmniphonySpatialObjectProcessor*);
using ProcessFn = std::int32_t (*)(
    OmniphonySpatialObjectProcessor*,
    const float*,
    const OmniphonySpatialDynamicObjectDescriptor*,
    std::uint32_t,
    const float*,
    float*,
    std::size_t);
using BlocksFn = std::uint64_t (*)(const OmniphonySpatialObjectProcessor*);
using LatencyFn = std::size_t (*)(const OmniphonySpatialObjectProcessor*);
using U32Fn = std::uint32_t (*)(const OmniphonySpatialObjectProcessor*);

using ResetFn = std::int32_t (*)(OmniphonySpatialObjectProcessor*);

template <typename T>
T Resolve(HMODULE module, const char* name) {
    return reinterpret_cast<T>(GetProcAddress(module, name));
}

bool AllFinite(const float* samples, std::size_t count) {
    for (std::size_t i = 0; i < count; ++i) {
        if (!std::isfinite(samples[i])) {
            return false;
        }
    }
    return true;
}

int Fail(const wchar_t* stage, int code) {
    std::wcerr << L"SPATIAL_OBJECT_REALTIME_SMOKE_FAIL stage=" << stage << L"\n";
    return code;
}

} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc != 2) {
        std::wcerr << L"usage: OmniphonySpatialObjectRealtimeSmoke.exe <omniphony_realtime.dll>\n";
        return 2;
    }

    HMODULE module = LoadLibraryW(argv[1]);
    if (!module) {
        std::wcerr << L"SPATIAL_OBJECT_REALTIME_LOAD_FAILED error=" << GetLastError() << L"\n";
        return 3;
    }

    const auto abiMajor = Resolve<AbiFn>(module, "omniphony_realtime_abi_major");
    const auto abiMinor = Resolve<AbiFn>(module, "omniphony_realtime_abi_minor");
    const auto create = Resolve<CreateFn>(module, "omniphony_spatial_objects_create");
    const auto destroy = Resolve<DestroyFn>(module, "omniphony_spatial_objects_destroy");
    const auto process = Resolve<ProcessFn>(module, "omniphony_spatial_objects_process_f32");
    const auto blocks = Resolve<BlocksFn>(module, "omniphony_spatial_objects_processed_blocks");
    const auto latency = Resolve<LatencyFn>(module, "omniphony_spatial_objects_latency_frames");
    const auto rate = Resolve<U32Fn>(module, "omniphony_spatial_objects_sample_rate_hz");
    const auto quantum = Resolve<U32Fn>(module, "omniphony_spatial_objects_frames_per_quantum");
    const auto staticCount = Resolve<U32Fn>(module, "omniphony_spatial_objects_static_object_count");
    const auto maxDynamic = Resolve<U32Fn>(module, "omniphony_spatial_objects_max_dynamic_objects");

    const auto reset = Resolve<ResetFn>(module, "omniphony_spatial_objects_reset");

    if (!abiMajor || !abiMinor || !create || !destroy || !process || !blocks ||
        !latency || !rate || !quantum || !staticCount || !maxDynamic || !reset) {
        FreeLibrary(module);
        return Fail(L"exports", 4);
    }
    if (abiMajor() != OMNIPHONY_REALTIME_ABI_MAJOR ||
        abiMinor() < OMNIPHONY_REALTIME_ABI_MINOR) {
        std::wcerr << L"SPATIAL_OBJECT_REALTIME_ABI_MISMATCH reported="
                   << abiMajor() << L"." << abiMinor() << L"\n";
        FreeLibrary(module);
        return 5;
    }

    constexpr std::size_t kFrames = 480;
    const OmniphonySpatialStaticObjectDescriptor staticDescriptor{
        OMNIPHONY_SPATIAL_STATIC_FRONT_LEFT,
        -0.5f,
        0.0f,
        -0.8660254f};
    const OmniphonySpatialObjectConfig config{
        48'000u,
        static_cast<std::uint32_t>(kFrames),
        1u,
        &staticDescriptor,
        2u};

    OmniphonySpatialObjectProcessor* processor = create(&config);
    if (!processor) {
        FreeLibrary(module);
        return Fail(L"create", 6);
    }

    int result = 0;
    if (rate(processor) != 48'000u ||
        quantum(processor) != kFrames ||
        staticCount(processor) != 1u ||
        maxDynamic(processor) != 2u ||
        latency(processor) != 1'920u) {
        result = Fail(L"contract", 7);
    }

    std::array<float, kFrames> staticPcm{};
    std::array<float, kFrames> dynamicPcm{};
    std::array<float, kFrames * 2> output{};
    staticPcm.fill(0.035f);
    dynamicPcm.fill(0.055f);

    OmniphonySpatialDynamicObjectDescriptor dynamic{
        0x4459'4E41'0000'0101ull,
        -0.9f,
        0.2f,
        -1.1f};

    bool sawNonzero = false;
    for (int pass = 0; pass < 14 && result == 0; ++pass) {
        const float t = static_cast<float>(pass) / 13.0f;
        dynamic.x_right_m = -0.9f + 1.8f * t;
        dynamic.y_up_m = 0.2f - 0.3f * t;
        dynamic.z_back_m = -1.1f + 0.7f * t;

        output.fill(NAN);
        if (process(
                processor,
                staticPcm.data(),
                &dynamic,
                1u,
                dynamicPcm.data(),
                output.data(),
                kFrames) != 0) {
            result = Fail(L"process-moving", 8);
            break;
        }
        if (!AllFinite(output.data(), output.size())) {
            result = Fail(L"nonfinite", 9);
            break;
        }
        sawNonzero = sawNonzero || std::any_of(
            output.begin(), output.end(), [](float sample) { return std::fabs(sample) > 1.0e-7f; });
        Sleep(15);
    }

    if (result == 0 && blocks(processor) == 0u) {
        result = Fail(L"worker-did-not-run", 10);
    }
    if (result == 0 && !sawNonzero) {
        result = Fail(L"no-output", 11);
    }

    if (result == 0) {
        // Exercise a dynamic-free quantum without changing the static topology.
        output.fill(NAN);
        if (process(
                processor,
                staticPcm.data(),
                nullptr,
                0u,
                nullptr,
                output.data(),
                kFrames) != 0 ||
            !AllFinite(output.data(), output.size())) {
            result = Fail(L"dynamic-free-quantum", 12);
        }
    }

    const auto processed = blocks(processor);
    if (result == 0) {
        if (reset(processor) != 0 ||
            blocks(processor) != 0u ||
            rate(processor) != 48'000u ||
            quantum(processor) != kFrames ||
            staticCount(processor) != 1u ||
            maxDynamic(processor) != 2u) {
            result = Fail(L"reset", 13);
        }
    }
    destroy(processor);
    FreeLibrary(module);

    if (result == 0) {
        std::wcout << L"SPATIAL_OBJECT_REALTIME_ABI_OK ABI="
                   << OMNIPHONY_REALTIME_ABI_MAJOR << L"."
                   << OMNIPHONY_REALTIME_ABI_MINOR
                   << L" STATIC=1 MAX_DYNAMIC=2 MOVING_XYZ=1 WORKER_BLOCKS="
                   << processed << L" RESET=1\n";
    }
    return result;
}
