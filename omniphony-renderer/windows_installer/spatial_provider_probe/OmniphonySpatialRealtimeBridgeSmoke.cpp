#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <mmreg.h>
#include <spatialaudioclient.h>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <iostream>
#include <memory>
#include <vector>

#include "OmniphonySpatialObjectRealtimeBridge.h"
#include "OmniphonySpatialRealtimeBridge.h"
#include "OmniphonySpatialRoles.h"
#include "OmniphonySpatialStereoQueue.h"

namespace {

int Fail(const wchar_t* stage, HRESULT hr) {
    std::wcerr << L"SPATIAL_REALTIME_BRIDGE_SMOKE_FAIL stage=" << stage
               << L" hr=0x" << std::hex << std::uppercase
               << static_cast<unsigned long>(hr) << std::dec << L"\n";
    return 1;
}

bool AllFinite(const std::vector<float>& samples) {
    return std::all_of(samples.begin(), samples.end(), [](float sample) {
        return std::isfinite(sample);
    });
}

WAVEFORMATEX ObjectFormat() {
    WAVEFORMATEX format{};
    format.wFormatTag = WAVE_FORMAT_IEEE_FLOAT;
    format.nChannels = 1;
    format.nSamplesPerSec = 48'000;
    format.wBitsPerSample = 32;
    format.nBlockAlign = sizeof(float);
    format.nAvgBytesPerSec = format.nSamplesPerSec * format.nBlockAlign;
    return format;
}

HRESULT FillObjectPattern(
    ISpatialAudioObject* object,
    UINT32 frames,
    std::uint32_t quantum,
    float amplitude) {
    if (!object) {
        return E_POINTER;
    }
    BYTE* bytes = nullptr;
    UINT32 byteCount = 0;
    HRESULT hr = object->GetBuffer(&bytes, &byteCount);
    if (FAILED(hr)) {
        return hr;
    }
    if (!bytes || byteCount != frames * sizeof(float)) {
        return E_FAIL;
    }

    auto* samples = reinterpret_cast<float*>(bytes);
    for (UINT32 frame = 0; frame < frames; ++frame) {
        const auto phase = (quantum * frames + frame) % 64;
        samples[frame] = phase < 32 ? amplitude : -amplitude;
    }
    return S_OK;
}

HRESULT ExerciseComToCurrentQueue(const wchar_t* realtimeDllPath) {
    auto format = ObjectFormat();
    SpatialAudioObjectRenderStreamActivationParams params{};
    params.ObjectFormat = &format;
    params.StaticObjectTypeMask = static_cast<AudioObjectType>(
        OmniphonySpatialObjectBits(AudioObjectType_FrontLeft) |
        OmniphonySpatialObjectBits(AudioObjectType_TopFrontLeft));
    params.MinDynamicObjectCount = 0;
    params.MaxDynamicObjectCount = 0;
    params.Category = AudioCategory_GameEffects;
    params.EventHandle = nullptr;
    params.NotifyObject = nullptr;

    auto queue = std::make_shared<OmniphonySpatialStereoQueue>();
    if (!queue->Open(1920)) {
        return E_OUTOFMEMORY;
    }

    ISpatialAudioObjectRenderStream* stream = nullptr;
    HRESULT hr = CreateOmniphonyStaticProbeStreamWithRealtimeBridgeAndQueue(
        params,
        realtimeDllPath,
        queue,
        &stream);
    if (FAILED(hr) || !stream) {
        return FAILED(hr) ? hr : E_FAIL;
    }

    ISpatialAudioObject* front = nullptr;
    ISpatialAudioObject* top = nullptr;
    hr = stream->ActivateSpatialAudioObject(AudioObjectType_FrontLeft, &front);
    if (FAILED(hr) || !front) {
        stream->Release();
        return FAILED(hr) ? hr : E_FAIL;
    }
    hr = stream->ActivateSpatialAudioObject(AudioObjectType_TopFrontLeft, &top);
    if (FAILED(hr) || !top) {
        front->Release();
        stream->Release();
        return FAILED(hr) ? hr : E_FAIL;
    }

    hr = stream->Start();
    if (FAILED(hr)) {
        top->Release();
        front->Release();
        stream->Release();
        return hr;
    }

    std::vector<float> queuedStereo(480 * 2, 0.0f);
    for (std::uint32_t quantum = 0; quantum < 16; ++quantum) {
        UINT32 available = 0;
        UINT32 frames = 0;
        hr = stream->BeginUpdatingAudioObjects(&available, &frames);
        if (FAILED(hr) || available != 0 || frames != 480) {
            if (SUCCEEDED(hr)) {
                hr = E_FAIL;
            }
            break;
        }

        BYTE* frontBytes = nullptr;
        UINT32 frontLength = 0;
        hr = front->GetBuffer(&frontBytes, &frontLength);
        if (FAILED(hr) || !frontBytes || frontLength != frames * sizeof(float)) {
            if (SUCCEEDED(hr)) {
                hr = E_FAIL;
            }
            break;
        }

        BYTE* topBytes = nullptr;
        UINT32 topLength = 0;
        hr = top->GetBuffer(&topBytes, &topLength);
        if (FAILED(hr) || !topBytes || topLength != frames * sizeof(float)) {
            if (SUCCEEDED(hr)) {
                hr = E_FAIL;
            }
            break;
        }

        auto* frontSamples = reinterpret_cast<float*>(frontBytes);
        auto* topSamples = reinterpret_cast<float*>(topBytes);
        for (UINT32 frame = 0; frame < frames; ++frame) {
            const auto phase = (quantum * frames + frame) % 64;
            frontSamples[frame] = phase < 32 ? 0.05f : -0.05f;
            topSamples[frame] = phase < 16 ? 0.04f : -0.04f;
        }

        hr = stream->EndUpdatingAudioObjects();
        if (FAILED(hr)) {
            break;
        }
        if (queue->AvailableFrames() != frames) {
            hr = E_FAIL;
            break;
        }

        std::fill(queuedStereo.begin(), queuedStereo.end(), 0.0f);
        const std::size_t readFrames = queue->Read(queuedStereo.data(), frames);
        if (readFrames != frames || !AllFinite(queuedStereo)) {
            hr = E_FAIL;
            break;
        }

        // Registry-free smoke only. Give Current's dedicated worker time to
        // consume the quantum without imposing a production scheduling model.
        Sleep(10);
    }

    if (SUCCEEDED(hr) &&
        (queue->AvailableFrames() != 0 || queue->DroppedFrames() != 0)) {
        hr = E_FAIL;
    }
    if (SUCCEEDED(hr)) {
        hr = stream->Stop();
    }

    top->Release();
    front->Release();
    stream->Release();
    queue->Close();
    return hr;
}


HRESULT ExerciseDynamicComToCurrentQueue(const wchar_t* realtimeDllPath) {
    auto format = ObjectFormat();
    SpatialAudioObjectRenderStreamActivationParams params{};
    params.ObjectFormat = &format;
    params.StaticObjectTypeMask = AudioObjectType_FrontLeft;
    params.MinDynamicObjectCount = 1;
    params.MaxDynamicObjectCount = 2;
    params.Category = AudioCategory_GameEffects;
    params.EventHandle = nullptr;
    params.NotifyObject = nullptr;

    auto queue = std::make_shared<OmniphonySpatialStereoQueue>();
    if (!queue->Open(1920)) {
        return E_OUTOFMEMORY;
    }

    ISpatialAudioObjectRenderStream* stream = nullptr;
    HRESULT hr = CreateOmniphonySpatialObjectStreamWithRealtimeBridgeAndQueue(
        params,
        realtimeDllPath,
        queue,
        &stream);
    if (FAILED(hr) || !stream) {
        queue->Close();
        return FAILED(hr) ? hr : E_FAIL;
    }

    hr = stream->Start();
    if (FAILED(hr)) {
        stream->Release();
        queue->Close();
        return hr;
    }

    ISpatialAudioObject* front = nullptr;
    ISpatialAudioObject* moving = nullptr;
    std::vector<float> queuedStereo(480 * 2, 0.0f);
    float peak = 0.0f;

    for (std::uint32_t quantum = 0; quantum < 16; ++quantum) {
        UINT32 available = 0;
        UINT32 frames = 0;
        hr = stream->BeginUpdatingAudioObjects(&available, &frames);
        if (FAILED(hr) || frames != 480) {
            if (SUCCEEDED(hr)) {
                hr = E_FAIL;
            }
            break;
        }

        if (!front) {
            hr = stream->ActivateSpatialAudioObject(AudioObjectType_FrontLeft, &front);
            if (FAILED(hr) || !front) {
                if (SUCCEEDED(hr)) {
                    hr = E_FAIL;
                }
                break;
            }
        }
        if (!moving) {
            hr = stream->ActivateSpatialAudioObject(AudioObjectType_Dynamic, &moving);
            if (FAILED(hr) || !moving) {
                if (SUCCEEDED(hr)) {
                    hr = E_FAIL;
                }
                break;
            }
        }

        const float t = static_cast<float>(quantum) / 15.0f;
        hr = moving->SetPosition(
            -0.8f + 1.6f * t,
            0.25f - 0.5f * t,
            -0.45f - 1.55f * t);
        if (FAILED(hr) ||
            FAILED(moving->SetVolume(0.65f)) ||
            FAILED(front->SetVolume(0.40f)) ||
            FAILED(FillObjectPattern(front, frames, quantum, 0.035f)) ||
            FAILED(FillObjectPattern(moving, frames, quantum, 0.055f))) {
            if (SUCCEEDED(hr)) {
                hr = E_FAIL;
            }
            break;
        }

        hr = stream->EndUpdatingAudioObjects();
        if (FAILED(hr)) {
            break;
        }
        if (queue->AvailableFrames() != frames) {
            hr = E_FAIL;
            break;
        }

        std::fill(queuedStereo.begin(), queuedStereo.end(), 0.0f);
        const std::size_t readFrames = queue->Read(queuedStereo.data(), frames);
        if (readFrames != frames || !AllFinite(queuedStereo)) {
            hr = E_FAIL;
            break;
        }
        for (float sample : queuedStereo) {
            peak = std::max(peak, std::abs(sample));
        }
        Sleep(12);
    }

    if (SUCCEEDED(hr) && (!(peak > 0.0f) || !std::isfinite(peak))) {
        hr = E_FAIL;
    }
    if (SUCCEEDED(hr) &&
        (queue->AvailableFrames() != 0 || queue->DroppedFrames() != 0)) {
        hr = E_FAIL;
    }
    if (SUCCEEDED(hr)) {
        hr = stream->Stop();
    }
    if (SUCCEEDED(hr)) {
        hr = stream->Reset();
    }
    if (SUCCEEDED(hr) && queue->AvailableFrames() != 0) {
        hr = E_FAIL;
    }

    if (moving) {
        moving->Release();
    }
    if (front) {
        front->Release();
    }
    stream->Release();
    queue->Close();
    return hr;
}

} // namespace

int wmain(int argc, wchar_t** argv) {
    if (argc != 2) {
        std::wcerr << L"usage: OmniphonySpatialRealtimeBridgeSmoke C:\\absolute\\path\\omniphony_realtime.dll\n";
        return 2;
    }

    constexpr std::uint32_t sampleRate = 48'000;
    constexpr std::uint32_t frames = 480;
    constexpr std::uint32_t objectCount = 2;

    const OmniphonySpatialStaticObjectDescriptor descriptors[objectCount] = {
        {
            OMNIPHONY_SPATIAL_STATIC_FRONT_LEFT,
            -kOmniphonySpatialDiagonal,
            0.0f,
            -kOmniphonySpatialDiagonal,
        },
        {
            OMNIPHONY_SPATIAL_STATIC_TOP_FRONT_LEFT,
            -0.5f,
            kOmniphonySpatialDiagonal,
            -0.5f,
        },
    };

    OmniphonySpatialRealtimeBridge bridge;
    HRESULT hr = bridge.Open(
        argv[1],
        sampleRate,
        frames,
        descriptors,
        objectCount);
    if (FAILED(hr) || !bridge.IsOpen()) {
        return Fail(L"Open", FAILED(hr) ? hr : E_FAIL);
    }

    if (bridge.LatencyFrames() == 0) {
        return Fail(L"LatencyFrames", E_FAIL);
    }

    std::vector<float> planar(static_cast<std::size_t>(frames) * objectCount);
    std::vector<float> stereo(static_cast<std::size_t>(frames) * 2);
    float peak = 0.0f;

    // First prove the narrow dynamic-loader ABI boundary directly.
    for (std::uint32_t quantum = 0; quantum < 16; ++quantum) {
        for (std::uint32_t frame = 0; frame < frames; ++frame) {
            const std::uint32_t phase = (quantum * frames + frame) % 64;
            planar[frame] = phase < 32 ? 0.05f : -0.05f;
            planar[frames + frame] = phase < 16 ? 0.04f : -0.04f;
        }
        std::fill(stereo.begin(), stereo.end(), 0.0f);

        hr = bridge.Process(planar.data(), stereo.data(), frames);
        if (FAILED(hr)) {
            return Fail(L"Process", hr);
        }
        if (!AllFinite(stereo)) {
            return Fail(L"finite-output", E_FAIL);
        }
        for (float sample : stereo) {
            peak = std::max(peak, std::abs(sample));
        }
        Sleep(10);
    }

    if (bridge.ProcessedBlocks() == 0) {
        return Fail(L"ProcessedBlocks", E_FAIL);
    }
    if (!(peak > 0.0f) || !std::isfinite(peak)) {
        return Fail(L"nonzero-output", E_FAIL);
    }

    const auto directLatency = bridge.LatencyFrames();
    const auto directBlocks = bridge.ProcessedBlocks();
    bridge.Close();

    // Then prove the composed registry-free path using the COM-shaped static
    // stream itself and the downstream clock-domain queue. This still does not
    // register/select Omniphony or touch a physical endpoint.
    hr = ExerciseComToCurrentQueue(argv[1]);
    if (FAILED(hr)) {
        return Fail(L"COM-to-Current-queue", hr);
    }

    hr = ExerciseDynamicComToCurrentQueue(argv[1]);
    if (FAILED(hr)) {
        return Fail(L"dynamic-COM-to-Current-queue", hr);
    }

    std::wcout << L"SPATIAL_REALTIME_BRIDGE_OK 1\n";
    std::wcout << L"SPATIAL_REALTIME_BRIDGE_OBJECTS " << objectCount << L"\n";
    std::wcout << L"SPATIAL_REALTIME_BRIDGE_FRAMES " << frames << L"\n";
    std::wcout << L"SPATIAL_REALTIME_BRIDGE_LATENCY_FRAMES "
               << directLatency << L"\n";
    std::wcout << L"SPATIAL_REALTIME_BRIDGE_PROCESSED_BLOCKS "
               << directBlocks << L"\n";
    std::wcout << L"SPATIAL_REALTIME_BRIDGE_OUTPUT_PEAK " << peak << L"\n";
    std::wcout << L"SPATIAL_COM_TO_CURRENT_OK 1\n";
    std::wcout << L"SPATIAL_COM_TO_STEREO_QUEUE_OK 1\n";
    std::wcout << L"SPATIAL_DYNAMIC_COM_TO_CURRENT_OK 1\n";
    std::wcout << L"SPATIAL_DYNAMIC_COM_TO_STEREO_QUEUE_OK 1\n";
    std::wcout << L"SPATIAL_FINAL_ENDPOINT_PROVEN 0\n";
    return 0;
}
