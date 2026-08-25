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

#include "OmniphonySpatialObjectStream.h"

namespace {

int Fail(const wchar_t* stage, HRESULT hr = E_FAIL) {
    std::wcerr << L"SPATIAL_OBJECT_STREAM_SMOKE_FAIL stage=" << stage
               << L" hr=0x" << std::hex << std::uppercase
               << static_cast<unsigned long>(hr) << std::dec << L"\n";
    return 1;
}

bool Near(float actual, float expected) {
    return std::abs(actual - expected) <= 1.0e-6f;
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

class RecordingTransport final : public OmniphonySpatialObjectQuantumTransport {
public:
    HRESULT Process(
        const float* staticInputPlanar,
        const OmniphonySpatialDynamicObjectDescriptor* dynamicObjects,
        std::uint32_t dynamicObjectCount,
        const float* dynamicInputPlanar,
        float* outputStereo,
        std::size_t frames) noexcept override {
        if (!outputStereo || frames != 480) {
            return E_INVALIDARG;
        }
        staticPlanar_.assign(staticInputPlanar, staticInputPlanar ? staticInputPlanar + frames : staticInputPlanar);
        dynamicDescriptors_.assign(
            dynamicObjects,
            dynamicObjects ? dynamicObjects + dynamicObjectCount : dynamicObjects);
        dynamicPlanar_.clear();
        if (dynamicInputPlanar && dynamicObjectCount > 0) {
            dynamicPlanar_.assign(
                dynamicInputPlanar,
                dynamicInputPlanar + static_cast<std::size_t>(dynamicObjectCount) * frames);
        }
        std::fill(outputStereo, outputStereo + frames * 2, 0.0f);
        ++calls_;
        return S_OK;
    }

    std::size_t Calls() const noexcept { return calls_; }
    const std::vector<float>& StaticPlanar() const noexcept { return staticPlanar_; }
    const std::vector<OmniphonySpatialDynamicObjectDescriptor>& Dynamic() const noexcept {
        return dynamicDescriptors_;
    }
    const std::vector<float>& DynamicPlanar() const noexcept { return dynamicPlanar_; }

private:
    std::vector<float> staticPlanar_;
    std::vector<OmniphonySpatialDynamicObjectDescriptor> dynamicDescriptors_;
    std::vector<float> dynamicPlanar_;
    std::size_t calls_ = 0;
};

HRESULT FillObject(ISpatialAudioObject* object, UINT32 frames, float value) {
    BYTE* bytes = nullptr;
    UINT32 byteCount = 0;
    const HRESULT hr = object->GetBuffer(&bytes, &byteCount);
    if (FAILED(hr)) {
        return hr;
    }
    if (!bytes || byteCount != frames * sizeof(float)) {
        return E_FAIL;
    }
    auto* samples = reinterpret_cast<float*>(bytes);
    std::fill(samples, samples + frames, value);
    return S_OK;
}

} // namespace

int wmain() {
    auto format = ObjectFormat();
    SpatialAudioObjectRenderStreamActivationParams params{};
    params.ObjectFormat = &format;
    params.StaticObjectTypeMask = AudioObjectType_FrontLeft;
    params.MinDynamicObjectCount = 1;
    params.MaxDynamicObjectCount = 2;
    params.Category = AudioCategory_GameEffects;

    std::shared_ptr<RecordingTransport> transport;
    try {
        transport = std::make_shared<RecordingTransport>();
    }
    catch (...) {
        return Fail(L"RecordingTransport", E_OUTOFMEMORY);
    }

    ISpatialAudioObjectRenderStream* stream = nullptr;
    HRESULT hr = CreateOmniphonySpatialObjectStreamWithTransport(params, transport, &stream);
    if (FAILED(hr) || !stream) {
        return Fail(L"CreateOmniphonySpatialObjectStreamWithTransport", hr);
    }

    UINT32 available = 0;
    hr = stream->GetAvailableDynamicObjectCount(&available);
    if (FAILED(hr) || available != 2) {
        stream->Release();
        return Fail(L"initial-dynamic-capacity", FAILED(hr) ? hr : E_FAIL);
    }

    hr = stream->Start();
    if (FAILED(hr)) {
        stream->Release();
        return Fail(L"Start", hr);
    }

    UINT32 frames = 0;
    hr = stream->BeginUpdatingAudioObjects(&available, &frames);
    if (FAILED(hr) || available != 2 || frames != 480) {
        stream->Release();
        return Fail(L"BeginUpdatingAudioObjects-first", FAILED(hr) ? hr : E_FAIL);
    }

    ISpatialAudioObject* front = nullptr;
    ISpatialAudioObject* movingA = nullptr;
    ISpatialAudioObject* movingB = nullptr;
    if (FAILED(stream->ActivateSpatialAudioObject(AudioObjectType_FrontLeft, &front)) || !front ||
        FAILED(stream->ActivateSpatialAudioObject(AudioObjectType_Dynamic, &movingA)) || !movingA ||
        FAILED(stream->ActivateSpatialAudioObject(AudioObjectType_Dynamic, &movingB)) || !movingB) {
        if (movingB) movingB->Release();
        if (movingA) movingA->Release();
        if (front) front->Release();
        stream->Release();
        return Fail(L"ActivateSpatialAudioObject");
    }

    ISpatialAudioObject* overflow = nullptr;
    hr = stream->ActivateSpatialAudioObject(AudioObjectType_Dynamic, &overflow);
    if (hr != SPTLAUDCLNT_E_NO_MORE_OBJECTS || overflow != nullptr) {
        movingB->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"dynamic-capacity-bound", hr);
    }

    if (FAILED(front->SetVolume(0.5f)) ||
        FAILED(movingA->SetVolume(0.25f)) ||
        FAILED(movingB->SetVolume(0.75f)) ||
        FAILED(movingA->SetPosition(-0.4f, 0.5f, -1.2f)) ||
        FAILED(movingB->SetPosition(0.8f, -0.3f, 0.2f)) ||
        FAILED(FillObject(front, frames, 0.2f)) ||
        FAILED(FillObject(movingA, frames, 0.6f)) ||
        FAILED(FillObject(movingB, frames, -0.4f))) {
        movingB->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"first-quantum-authoring");
    }

    hr = stream->EndUpdatingAudioObjects();
    if (FAILED(hr) || transport->Calls() != 1 || transport->Dynamic().size() != 2) {
        movingB->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"EndUpdatingAudioObjects-first", FAILED(hr) ? hr : E_FAIL);
    }

    const auto firstA = transport->Dynamic()[0];
    const auto firstB = transport->Dynamic()[1];
    if (firstA.stable_id == 0 || firstB.stable_id == 0 || firstA.stable_id == firstB.stable_id ||
        !Near(firstA.x_right_m, -0.4f) || !Near(firstA.y_up_m, 0.5f) || !Near(firstA.z_back_m, -1.2f) ||
        !Near(firstB.x_right_m, 0.8f) || !Near(firstB.y_up_m, -0.3f) || !Near(firstB.z_back_m, 0.2f) ||
        transport->StaticPlanar().size() != 480 ||
        !Near(transport->StaticPlanar()[0], 0.1f) ||
        transport->DynamicPlanar().size() != 960 ||
        !Near(transport->DynamicPlanar()[0], 0.15f) ||
        !Near(transport->DynamicPlanar()[480], -0.3f)) {
        movingB->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"first-quantum-contract");
    }

    hr = stream->BeginUpdatingAudioObjects(&available, &frames);
    if (FAILED(hr) || available != 0) {
        movingB->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"held-object-capacity", FAILED(hr) ? hr : E_FAIL);
    }

    if (FAILED(movingA->SetPosition(0.3f, 0.1f, -0.7f)) ||
        FAILED(FillObject(movingA, frames, 0.5f))) {
        movingB->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"second-quantum-authoring");
    }

    hr = stream->EndUpdatingAudioObjects();
    if (FAILED(hr) || transport->Calls() != 2 || transport->Dynamic().size() != 1) {
        movingB->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"EndUpdatingAudioObjects-second", FAILED(hr) ? hr : E_FAIL);
    }

    const auto secondA = transport->Dynamic()[0];
    if (secondA.stable_id != firstA.stable_id ||
        !Near(secondA.x_right_m, 0.3f) || !Near(secondA.y_up_m, 0.1f) || !Near(secondA.z_back_m, -0.7f)) {
        movingB->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"stable-id-moving-position");
    }

    BOOL active = TRUE;
    hr = movingB->IsActive(&active);
    if (FAILED(hr) || active) {
        movingB->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"skipped-buffer-ends-object", FAILED(hr) ? hr : E_FAIL);
    }

    // Capacity remains held until the application releases its dynamic object,
    // even after the object's stream lifetime has ended.
    hr = stream->GetAvailableDynamicObjectCount(&available);
    if (FAILED(hr) || available != 0) {
        movingB->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"dead-but-held-capacity", FAILED(hr) ? hr : E_FAIL);
    }
    movingB->Release();
    movingB = nullptr;
    hr = stream->GetAvailableDynamicObjectCount(&available);
    if (FAILED(hr) || available != 1) {
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"release-restores-capacity", FAILED(hr) ? hr : E_FAIL);
    }

    hr = stream->BeginUpdatingAudioObjects(&available, &frames);
    if (FAILED(hr) || available != 1) {
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"BeginUpdatingAudioObjects-third", FAILED(hr) ? hr : E_FAIL);
    }

    ISpatialAudioObject* movingC = nullptr;
    hr = stream->ActivateSpatialAudioObject(AudioObjectType_Dynamic, &movingC);
    if (FAILED(hr) || !movingC ||
        FAILED(FillObject(movingA, frames, 0.25f)) ||
        FAILED(FillObject(movingC, frames, 0.1f))) {
        if (movingC) movingC->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"third-quantum-authoring", FAILED(hr) ? hr : E_FAIL);
    }

    hr = stream->EndUpdatingAudioObjects();
    if (FAILED(hr) || transport->Dynamic().size() != 2) {
        movingC->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"EndUpdatingAudioObjects-third", FAILED(hr) ? hr : E_FAIL);
    }

    const auto thirdA = transport->Dynamic()[0];
    const auto thirdC = transport->Dynamic()[1];
    if (thirdA.stable_id != firstA.stable_id ||
        thirdC.stable_id == firstA.stable_id || thirdC.stable_id == firstB.stable_id ||
        !Near(thirdA.x_right_m, 0.3f) || !Near(thirdA.y_up_m, 0.1f) || !Near(thirdA.z_back_m, -0.7f) ||
        !Near(thirdC.x_right_m, 0.0f) || !Near(thirdC.y_up_m, 0.0f) || !Near(thirdC.z_back_m, 0.0f)) {
        movingC->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"identity-position-persistence-and-origin");
    }

    hr = stream->Stop();
    if (FAILED(hr)) {
        movingC->Release();
        movingA->Release();
        front->Release();
        stream->Release();
        return Fail(L"Stop", hr);
    }

    movingC->Release();
    movingA->Release();
    front->Release();
    stream->Release();

    std::wcout << L"SPATIAL_OBJECT_STREAM_DYNAMIC_CAPACITY_OK 1\n";
    std::wcout << L"SPATIAL_OBJECT_STREAM_STABLE_ID_OK 1\n";
    std::wcout << L"SPATIAL_OBJECT_STREAM_XYZ_PERSISTENCE_OK 1\n";
    std::wcout << L"SPATIAL_OBJECT_STREAM_RELEASE_RECLAIM_OK 1\n";
    return 0;
}
