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

class OmniphonySpatialObjectQuantumTransport {
public:
    virtual ~OmniphonySpatialObjectQuantumTransport() = default;

    virtual HRESULT Process(
        const float* staticInputPlanar,
        const OmniphonySpatialDynamicObjectDescriptor* dynamicObjects,
        std::uint32_t dynamicObjectCount,
        const float* dynamicInputPlanar,
        float* outputStereo,
        std::size_t frames) noexcept = 0;
};

HRESULT CreateOmniphonySpatialObjectStreamWithTransport(
    const SpatialAudioObjectRenderStreamActivationParams& params,
    std::shared_ptr<OmniphonySpatialObjectQuantumTransport> transport,
    ISpatialAudioObjectRenderStream** stream);
