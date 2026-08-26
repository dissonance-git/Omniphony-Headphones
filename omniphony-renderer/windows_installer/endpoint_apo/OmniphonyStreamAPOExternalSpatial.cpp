// External-spatial handoff shim for the Omniphony stream SFX.
//
// The core Stream APO already has a proven RAW identity path. Windows also
// exposes the active Spatial Sound format for the exact endpoint handed to an
// APO through APOInitSystemEffects3. Reuse the identity path whenever another
// spatial renderer (Windows Sonic, Dolby Atmos for Headphones, DTS, or the
// experimental Omniphony provider) already owns binaural presentation.
//
// Spatial Sound Off leaves the original Stream APO behavior untouched.

#include <windows.h>
#include <unknwn.h>
#include <audioenginebaseapo.h>
#include <audioengineextensionapo.h>
#include <audiomediatype.h>
#include <BaseAudioProcessingObject.h>
#include <ksmedia.h>
#include <mmdeviceapi.h>
#include <wrl/client.h>
#include <winrt/base.h>
#include <winrt/Windows.Media.Audio.h>

namespace {

const GUID kOmniphonyOriginalRawMode = AUDIO_SIGNALPROCESSINGMODE_RAW;

bool OmniphonyExternalSpatialActive(UINT32 dataSize, BYTE* data) noexcept {
    if (!data || dataSize != sizeof(APOInitSystemEffects3)) {
        return false;
    }

    auto* init = reinterpret_cast<APOInitSystemEffects3*>(data);
    if (init->InitializeForDiscoveryOnly || !init->pDeviceCollection) {
        return false;
    }

    try {
        UINT count = 0;
        if (FAILED(init->pDeviceCollection->GetCount(&count)) || count == 0) {
            return false;
        }

        Microsoft::WRL::ComPtr<IMMDevice> endpoint;
        if (FAILED(init->pDeviceCollection->Item(count - 1, endpoint.ReleaseAndGetAddressOf())) ||
            !endpoint) {
            return false;
        }

        LPWSTR endpointId = nullptr;
        if (FAILED(endpoint->GetId(&endpointId)) || !endpointId) {
            return false;
        }

        winrt::hstring id{endpointId};
        CoTaskMemFree(endpointId);
        endpointId = nullptr;

        const auto configuration =
            winrt::Windows::Media::Audio::SpatialAudioDeviceConfiguration::GetForDeviceId(id);
        const auto active = configuration.ActiveSpatialAudioFormat();
        return !active.empty();
    } catch (...) {
        // Fail toward Omniphony rather than muting or breaking graph creation.
        // A query failure must never make the SFX reject an otherwise-valid
        // normal stream.
        return false;
    }
}

GUID OmniphonyRawOrExternalSpatialMode(
    const GUID& processingMode,
    UINT32 dataSize,
    BYTE* data) noexcept {
    if (IsEqualGUID(processingMode, kOmniphonyOriginalRawMode) ||
        OmniphonyExternalSpatialActive(dataSize, data)) {
        return processingMode;
    }
    return kOmniphonyOriginalRawMode;
}

} // namespace

// The original source references AUDIO_SIGNALPROCESSINGMODE_RAW exactly once,
// in Initialize(), where processingMode/dataSize/data are all in scope. Replace
// only that token so the existing, heavily-tested RAW identity implementation
// also becomes the external-spatial identity implementation.
#define AUDIO_SIGNALPROCESSINGMODE_RAW \
    OmniphonyRawOrExternalSpatialMode(processingMode, dataSize, data)
#include "OmniphonyStreamAPO.cpp"
#undef AUDIO_SIGNALPROCESSINGMODE_RAW
