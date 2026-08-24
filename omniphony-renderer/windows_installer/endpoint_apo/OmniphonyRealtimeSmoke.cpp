#include <windows.h>

#include "omniphony_realtime.h"

#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <iostream>

namespace {
using AbiFn = uint32_t (*)();
using CreateFn = OmniphonyRealtimeProcessor* (*)(const OmniphonyRealtimeConfig*);
using DestroyFn = void (*)(OmniphonyRealtimeProcessor*);
using SetModeFn = int32_t (*)(OmniphonyRealtimeProcessor*, uint32_t);
using ModeFn = uint32_t (*)(const OmniphonyRealtimeProcessor*);
using ProcessFn = int32_t (*)(OmniphonyRealtimeProcessor*, const float*, float*, size_t);
using BlocksFn = uint64_t (*)(const OmniphonyRealtimeProcessor*);
using RenderedFramesFn = uint64_t (*)(const OmniphonyRealtimeProcessor*);
using LatencyFramesFn = size_t (*)(const OmniphonyRealtimeProcessor*);

using SpatialStaticCreateFn = OmniphonySpatialStaticProcessor* (*)(const OmniphonySpatialStaticConfig*);
using SpatialStaticDestroyFn = void (*)(OmniphonySpatialStaticProcessor*);
using SpatialStaticProcessFn = int32_t (*)(OmniphonySpatialStaticProcessor*, const float*, float*, size_t);
using SpatialStaticBlocksFn = uint64_t (*)(const OmniphonySpatialStaticProcessor*);
using SpatialStaticLatencyFn = size_t (*)(const OmniphonySpatialStaticProcessor*);
using SpatialStaticU32Fn = uint32_t (*)(const OmniphonySpatialStaticProcessor*);

template <typename T>
T Resolve(HMODULE module, const char* name) {
    return reinterpret_cast<T>(GetProcAddress(module, name));
}

bool AllFinite(const float* samples, size_t count) {
    for (size_t i = 0; i < count; ++i) {
        if (!std::isfinite(samples[i])) {
            return false;
        }
    }
    return true;
}
} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc != 2) {
        std::wcerr << L"usage: OmniphonyRealtimeSmoke.exe <omniphony_realtime.dll>" << std::endl;
        return 2;
    }

    HMODULE module = LoadLibraryW(argv[1]);
    if (!module) {
        std::wcerr << L"REALTIME_LOAD_FAILED\t" << GetLastError() << std::endl;
        return 3;
    }

    const auto abiMajor = Resolve<AbiFn>(module, "omniphony_realtime_abi_major");
    const auto abiMinor = Resolve<AbiFn>(module, "omniphony_realtime_abi_minor");
    const auto create = Resolve<CreateFn>(module, "omniphony_realtime_create");
    const auto destroy = Resolve<DestroyFn>(module, "omniphony_realtime_destroy");
    const auto setMode = Resolve<SetModeFn>(module, "omniphony_realtime_set_mode");
    const auto mode = Resolve<ModeFn>(module, "omniphony_realtime_mode");
    const auto process = Resolve<ProcessFn>(module, "omniphony_realtime_process_f32");
    const auto blocks = Resolve<BlocksFn>(module, "omniphony_realtime_processed_blocks");
    const auto renderedFrames = Resolve<RenderedFramesFn>(module, "omniphony_realtime_rendered_frames");
    const auto latencyFrames = Resolve<LatencyFramesFn>(module, "omniphony_realtime_latency_frames");

    const auto spatialCreate = Resolve<SpatialStaticCreateFn>(module, "omniphony_spatial_static_create");
    const auto spatialDestroy = Resolve<SpatialStaticDestroyFn>(module, "omniphony_spatial_static_destroy");
    const auto spatialProcess = Resolve<SpatialStaticProcessFn>(module, "omniphony_spatial_static_process_f32");
    const auto spatialBlocks = Resolve<SpatialStaticBlocksFn>(module, "omniphony_spatial_static_processed_blocks");
    const auto spatialLatency = Resolve<SpatialStaticLatencyFn>(module, "omniphony_spatial_static_latency_frames");
    const auto spatialRate = Resolve<SpatialStaticU32Fn>(module, "omniphony_spatial_static_sample_rate_hz");
    const auto spatialQuantum = Resolve<SpatialStaticU32Fn>(module, "omniphony_spatial_static_frames_per_quantum");
    const auto spatialCount = Resolve<SpatialStaticU32Fn>(module, "omniphony_spatial_static_object_count");

    if (!abiMajor || !abiMinor || !create || !destroy || !setMode || !mode ||
        !process || !blocks || !renderedFrames || !latencyFrames || !spatialCreate || !spatialDestroy ||
        !spatialProcess || !spatialBlocks || !spatialLatency || !spatialRate ||
        !spatialQuantum || !spatialCount) {
        std::wcerr << L"REALTIME_EXPORTS_MISSING" << std::endl;
        FreeLibrary(module);
        return 4;
    }
    if (abiMajor() != OMNIPHONY_REALTIME_ABI_MAJOR ||
        abiMinor() < OMNIPHONY_REALTIME_ABI_MINOR) {
        std::wcerr << L"REALTIME_ABI_MISMATCH\t" << abiMajor() << L"." << abiMinor() << std::endl;
        FreeLibrary(module);
        return 5;
    }

    const OmniphonyRealtimeConfig config{48000u, 2u};
    OmniphonyRealtimeProcessor* processor = create(&config);
    if (!processor) {
        std::wcerr << L"REALTIME_CREATE_FAILED" << std::endl;
        FreeLibrary(module);
        return 6;
    }

    int result = 0;
    if (setMode(processor, OMNIPHONY_REALTIME_MODE_IDENTITY) != 0 ||
        mode(processor) != OMNIPHONY_REALTIME_MODE_IDENTITY ||
        latencyFrames(processor) != 0u) {
        std::wcerr << L"REALTIME_IDENTITY_MODE_FAILED" << std::endl;
        result = 7;
    } else {
        const std::array<float, 8> input = {0.0f, -0.25f, 0.5f, 1.0f, -1.0f, 0.125f, -0.75f, 0.875f};
        std::array<float, 8> output = {};
        if (process(processor, input.data(), output.data(), 4u) != 0 ||
            std::memcmp(input.data(), output.data(), sizeof(input)) != 0) {
            std::wcerr << L"REALTIME_IDENTITY_PROCESS_FAILED" << std::endl;
            result = 8;
        } else if (blocks(processor) != 0u) {
            std::wcerr << L"REALTIME_IDENTITY_BLOCK_COUNTER_CHANGED" << std::endl;
            result = 9;
        } else if (setMode(processor, OMNIPHONY_REALTIME_MODE_CURRENT) != 0 ||
                   mode(processor) != OMNIPHONY_REALTIME_MODE_CURRENT) {
            std::wcerr << L"REALTIME_CURRENT_MODE_FAILED" << std::endl;
            result = 10;
        } else if (latencyFrames(processor) != 1920u) {
            std::wcerr << L"REALTIME_CURRENT_LATENCY_FAILED\tFRAMES="
                       << latencyFrames(processor) << std::endl;
            result = 11;
        } else {
            // Drive the optimized DLL the way an audio host does while its
            // Current worker initializes asynchronously. This must prove that
            // real rendered PCM crosses back into the callback; initialization
            // alone is not a sufficient release contract.
            constexpr size_t kCurrentFrames = 960;
            constexpr float kOutputCeiling = 0.8912509f;
            std::array<float, kCurrentFrames * 2> currentInput = {};
            std::array<float, kCurrentFrames * 2> currentOutput = {};
            for (size_t frame = 0; frame < kCurrentFrames; ++frame) {
                currentInput[frame * 2] = (frame % 2 == 0) ? 2.0f : -2.0f;
                currentInput[frame * 2 + 1] = (frame % 3 == 0) ? -1.75f : 1.75f;
            }

            bool crossed = false;
            const ULONGLONG deadline = GetTickCount64() + 20000u;
            while (GetTickCount64() < deadline && result == 0) {
                currentOutput.fill(NAN);
                if (process(processor, currentInput.data(), currentOutput.data(), kCurrentFrames) != 0) {
                    std::wcerr << L"REALTIME_CURRENT_PROCESS_FAILED" << std::endl;
                    result = 18;
                    break;
                }
                if (!AllFinite(currentOutput.data(), currentOutput.size())) {
                    std::wcerr << L"REALTIME_CURRENT_NONFINITE_OUTPUT" << std::endl;
                    result = 19;
                    break;
                }

                float peak = 0.0f;
                bool nonzero = false;
                for (const float sample : currentOutput) {
                    peak = (std::max)(peak, std::fabs(sample));
                    nonzero = nonzero || std::fabs(sample) > 1.0e-6f;
                }
                if (peak > kOutputCeiling + 1.0e-6f) {
                    std::wcerr << L"REALTIME_CURRENT_CEILING_FAILED\tPEAK=" << peak << std::endl;
                    result = 20;
                    break;
                }
                if (blocks(processor) > 0u && renderedFrames(processor) > 0u && nonzero) {
                    crossed = true;
                    break;
                }
                Sleep(20);
            }
            if (result == 0 && !crossed) {
                std::wcerr << L"REALTIME_CURRENT_RENDER_TIMEOUT\tBLOCKS=" << blocks(processor)
                           << L"\tRENDERED_FRAMES=" << renderedFrames(processor) << std::endl;
                result = 21;
            } else if (result == 0) {
                std::wcout << L"REALTIME_CURRENT_RENDER_OK\tBLOCKS=" << blocks(processor)
                           << L"\tRENDERED_FRAMES=" << renderedFrames(processor)
                           << L"\tCEILING=" << kOutputCeiling << std::endl;
            }
        }
    }
    destroy(processor);

    if (result == 0) {
        constexpr size_t kQuantum = 480;
        const OmniphonySpatialStaticObjectDescriptor descriptor{
            OMNIPHONY_SPATIAL_STATIC_TOP_FRONT_LEFT,
            -0.5f,
            0.70710678f,
            -0.5f};
        const OmniphonySpatialStaticConfig spatialConfig{
            48000u,
            static_cast<uint32_t>(kQuantum),
            1u,
            &descriptor};

        OmniphonySpatialStaticProcessor* spatial = spatialCreate(&spatialConfig);
        if (!spatial) {
            std::wcerr << L"SPATIAL_STATIC_CREATE_FAILED" << std::endl;
            result = 12;
        } else if (spatialRate(spatial) != 48000u ||
                   spatialQuantum(spatial) != kQuantum ||
                   spatialCount(spatial) != 1u ||
                   spatialLatency(spatial) != 1920u) {
            std::wcerr << L"SPATIAL_STATIC_CONTRACT_FAILED" << std::endl;
            result = 13;
        } else {
            std::array<float, kQuantum> input = {};
            std::array<float, kQuantum * 2> output = {};
            input.fill(0.05f);

            for (int quantum = 0; quantum < 8 && result == 0; ++quantum) {
                output.fill(0.0f);
                if (spatialProcess(spatial, input.data(), output.data(), kQuantum) != 0) {
                    std::wcerr << L"SPATIAL_STATIC_PROCESS_FAILED\tQUANTUM=" << quantum << std::endl;
                    result = 14;
                    break;
                }
                if (!AllFinite(output.data(), output.size())) {
                    std::wcerr << L"SPATIAL_STATIC_NONFINITE_OUTPUT\tQUANTUM=" << quantum << std::endl;
                    result = 15;
                    break;
                }
                Sleep(15);
            }

            if (result == 0 && spatialBlocks(spatial) == 0u) {
                std::wcerr << L"SPATIAL_STATIC_WORKER_DID_NOT_RUN" << std::endl;
                result = 16;
            }
            if (result == 0) {
                std::wcout << L"SPATIAL_STATIC_ABI_OK\tOBJECTS=1\tROLE=TFL"
                           << L"\tRATE=48000\tQUANTUM=480\tLATENCY_FRAMES=1920"
                           << L"\tWORKER_BLOCKS=" << spatialBlocks(spatial)
                           << std::endl;
            }
        }
        if (spatial) {
            spatialDestroy(spatial);
        }
    }

    const uint32_t reportedAbiMajor = abiMajor();
    const uint32_t reportedAbiMinor = abiMinor();
    if (result == 0) {
        std::wcout << L"REALTIME_DLL_OK\tABI=" << reportedAbiMajor << L"." << reportedAbiMinor
                   << L"\tIDENTITY_BIT_EXACT=1\tCURRENT_INIT=1\tCURRENT_LATENCY_FRAMES=1920"
                   << L"\tSPATIAL_STATIC_INIT=1"
                   << std::endl;
    }

    FreeLibrary(module);
    module = nullptr;

    if (result == 0) {
        HMODULE resident = nullptr;
        const BOOL residentByAddress = GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS |
                GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            reinterpret_cast<LPCWSTR>(abiMajor),
            &resident);
        if (!residentByAddress || resident == nullptr) {
            std::wcerr << L"REALTIME_DLL_PIN_FAILED" << std::endl;
            result = 17;
        } else {
            std::wcout << L"REALTIME_DLL_PIN_OK\tPROCESS_LIFETIME=1" << std::endl;
        }
    }
    return result;
}
