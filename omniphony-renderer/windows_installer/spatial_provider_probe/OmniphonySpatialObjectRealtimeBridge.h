#pragma once

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <spatialaudioclient.h>

#include <cstddef>
#include <cstdint>
#include <memory>

#include "omniphony_realtime.h"

class OmniphonySpatialStereoQueue;

HRESULT CreateOmniphonySpatialObjectStreamWithRealtimeBridgeAndQueue(
    const SpatialAudioObjectRenderStreamActivationParams& params,
    const wchar_t* realtimeDllPath,
    std::shared_ptr<OmniphonySpatialStereoQueue> stereoQueue,
    ISpatialAudioObjectRenderStream** stream);
