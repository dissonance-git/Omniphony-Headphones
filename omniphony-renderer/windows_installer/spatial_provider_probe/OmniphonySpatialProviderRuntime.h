#pragma once

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <propidl.h>

// Creates the live static-object provider stream used only when the installed
// Omniphony provider runtime is explicitly enabled. The activation blob remains
// the Windows-owned SpatialAudioObjectRenderStreamActivationParams contract.
//
// The factory binds one application-supplied static object stream to the existing
// Current renderer, a bounded stereo cadence queue, and one exact physical RAW
// headphone endpoint. Dynamic objects remain unavailable until their continuous
// XYZ transport has a truthful realtime ABI of its own.
HRESULT CreateOmniphonySpatialProviderStaticStreamFromActivation(
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
