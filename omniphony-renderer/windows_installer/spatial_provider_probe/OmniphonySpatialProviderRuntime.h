#pragma once

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <propidl.h>

// Creates the live Windows Spatial Audio object stream used only when the
// installed Omniphony provider runtime is explicitly enabled. The activation
// blob remains the Windows-owned SpatialAudioObjectRenderStreamActivationParams
// contract.
//
// Static roles and dynamic objects share one source-authoritative renderer and
// one binaural output pass. Dynamic objects retain stable stream-local identity
// and continuous listener-relative XYZ instead of being quantized to the static
// 17-role bed. The final stereo stream crosses the bounded cadence queue and one
// exact physical RAW headphone endpoint.
HRESULT CreateOmniphonySpatialProviderObjectStreamFromActivation(
    const PROPVARIANT* activationParams,
    REFIID riid,
    const wchar_t* realtimeDllPath,
    const wchar_t* physicalEndpointId,
    void** stream);

// A public provider stream may outlive the ISpatialAudioClient that created it.
// These hooks keep DllCanUnloadNow tied to the returned stream lifetime instead
// of only to the client/class-factory lifetime.
void OmniphonySpatialProviderModuleAddRef() noexcept;
void OmniphonySpatialProviderModuleRelease() noexcept;
