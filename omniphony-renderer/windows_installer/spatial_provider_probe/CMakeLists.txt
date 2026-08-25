cmake_minimum_required(VERSION 3.20)
project(OmniphonySpatialProviderProbe LANGUAGES CXX)

if(NOT WIN32)
    message(FATAL_ERROR "The Omniphony Spatial Provider Probe is Windows-only.")
endif()

set(OMNIPHONY_REALTIME_INCLUDE
    "${CMAKE_CURRENT_LIST_DIR}/../../realtime_ffi/include"
)

add_library(OmniphonySpatialProbe SHARED
    OmniphonySpatialProbe.cpp
    OmniphonySpatialProviderRuntime.cpp
    OmniphonySpatialStaticStream.cpp
    OmniphonySpatialRealtimeBridge.cpp
    OmniphonySpatialRawOutputPump.cpp
    OmniphonySpatialRawOutputSink.cpp
    OmniphonySpatialStereoQueue.cpp
    OmniphonySpatialProbe.def
)
target_compile_features(OmniphonySpatialProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialProbe PRIVATE UNICODE _UNICODE)
target_compile_options(OmniphonySpatialProbe PRIVATE /W4 /WX /permissive-)
target_include_directories(OmniphonySpatialProbe PRIVATE "${OMNIPHONY_REALTIME_INCLUDE}")
target_link_libraries(OmniphonySpatialProbe PRIVATE ole32 uuid advapi32 avrt)
set_target_properties(OmniphonySpatialProbe PROPERTIES
    PREFIX ""
    OUTPUT_NAME "OmniphonySpatialProbe"
)

add_executable(OmniphonySpatialProbeCtl
    OmniphonySpatialProbeCtl.cpp
)
target_compile_features(OmniphonySpatialProbeCtl PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialProbeCtl PRIVATE UNICODE _UNICODE)
target_compile_options(OmniphonySpatialProbeCtl PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonySpatialProbeCtl PRIVATE ole32 uuid advapi32)

add_executable(OmniphonySpatialProbeSmoke
    OmniphonySpatialProbeSmoke.cpp
)
target_compile_features(OmniphonySpatialProbeSmoke PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialProbeSmoke PRIVATE UNICODE _UNICODE)
target_compile_options(OmniphonySpatialProbeSmoke PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonySpatialProbeSmoke PRIVATE ole32 uuid)

add_executable(OmniphonySpatialStaticStreamSmoke
    OmniphonySpatialStaticStreamSmoke.cpp
    OmniphonySpatialStaticStream.cpp
)
target_compile_features(OmniphonySpatialStaticStreamSmoke PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialStaticStreamSmoke PRIVATE UNICODE _UNICODE)
target_compile_options(OmniphonySpatialStaticStreamSmoke PRIVATE /W4 /WX /permissive-)
target_include_directories(
    OmniphonySpatialStaticStreamSmoke
    PRIVATE "${OMNIPHONY_REALTIME_INCLUDE}"
)
target_link_libraries(OmniphonySpatialStaticStreamSmoke PRIVATE ole32 uuid)

add_executable(OmniphonySpatialRealtimeBridgeSmoke
    OmniphonySpatialRealtimeBridgeSmoke.cpp
    OmniphonySpatialRealtimeBridge.cpp
    OmniphonySpatialStaticStream.cpp
    OmniphonySpatialStereoQueue.cpp
)
target_compile_features(OmniphonySpatialRealtimeBridgeSmoke PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialRealtimeBridgeSmoke PRIVATE UNICODE _UNICODE)
target_compile_options(OmniphonySpatialRealtimeBridgeSmoke PRIVATE /W4 /WX /permissive-)
target_include_directories(
    OmniphonySpatialRealtimeBridgeSmoke
    PRIVATE "${OMNIPHONY_REALTIME_INCLUDE}"
)
target_link_libraries(OmniphonySpatialRealtimeBridgeSmoke PRIVATE ole32 uuid)

# Read-only physical-endpoint preflight. This deliberately never initializes a
# render stream: it only proves endpoint identity, RAW client properties,
# stereo float support, and shared-engine period constraints before a future
# installer transaction is allowed to activate/select the spatial provider.
add_executable(OmniphonySpatialRawOutputProbe
    OmniphonySpatialRawOutputProbe.cpp
)
target_compile_features(OmniphonySpatialRawOutputProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialRawOutputProbe PRIVATE UNICODE _UNICODE)
target_compile_options(OmniphonySpatialRawOutputProbe PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonySpatialRawOutputProbe PRIVATE ole32 uuid)

# Provider-egress control-path primitive. It opens the exact physical endpoint
# as a shared event-driven RAW stereo stream, obtains IAudioRenderClient, and
# binds the endpoint event. It intentionally exposes no public Start() operation,
# so install preflight can prove the lifecycle without creating playback.
add_executable(OmniphonySpatialRawOutputSinkProbe
    OmniphonySpatialRawOutputSinkProbe.cpp
    OmniphonySpatialRawOutputSink.cpp
)
target_compile_features(OmniphonySpatialRawOutputSinkProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialRawOutputSinkProbe PRIVATE UNICODE _UNICODE)
target_compile_options(OmniphonySpatialRawOutputSinkProbe PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonySpatialRawOutputSinkProbe PRIVATE ole32 uuid)

# Pure clock-domain adapter smoke. The Current worker keeps its fixed 480-frame
# producer quantum while the eventual endpoint consumer may request any legal
# shared-engine period. The queue is preallocated and SPSC so neither side has
# to block or allocate after startup.
add_executable(OmniphonySpatialStereoQueueSmoke
    OmniphonySpatialStereoQueueSmoke.cpp
    OmniphonySpatialStereoQueue.cpp
)
target_compile_features(OmniphonySpatialStereoQueueSmoke PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialStereoQueueSmoke PRIVATE UNICODE _UNICODE)
target_compile_options(OmniphonySpatialStereoQueueSmoke PRIVATE /W4 /WX /permissive-)

# Compile/link and fail-closed lifecycle smoke for the active endpoint-event
# pump. The smoke never opens a physical endpoint, starts IAudioClient, or opens
# the public provider gate. Physical output remains a separate evidence step.
add_executable(OmniphonySpatialRawOutputPumpSmoke
    OmniphonySpatialRawOutputPumpSmoke.cpp
    OmniphonySpatialRawOutputPump.cpp
    OmniphonySpatialRawOutputSink.cpp
    OmniphonySpatialStereoQueue.cpp
)
target_compile_features(OmniphonySpatialRawOutputPumpSmoke PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialRawOutputPumpSmoke PRIVATE UNICODE _UNICODE)
target_compile_options(OmniphonySpatialRawOutputPumpSmoke PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonySpatialRawOutputPumpSmoke PRIVATE ole32 uuid)

# Manual physical egress diagnostic. CI compiles this target but must never run
# it because it requires an explicit real endpoint and intentionally produces a
# short low-level audible test signal. It still does no provider registration or
# provider selection and cannot open the public Spatial Audio stream gate.
add_executable(OmniphonySpatialClosedGateEgressProbe
    OmniphonySpatialClosedGateEgressProbe.cpp
    OmniphonySpatialRawOutputPump.cpp
    OmniphonySpatialRawOutputSink.cpp
    OmniphonySpatialRealtimeBridge.cpp
    OmniphonySpatialStaticStream.cpp
    OmniphonySpatialStereoQueue.cpp
)
target_compile_features(OmniphonySpatialClosedGateEgressProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialClosedGateEgressProbe PRIVATE UNICODE _UNICODE)
target_compile_options(OmniphonySpatialClosedGateEgressProbe PRIVATE /W4 /WX /permissive-)
target_include_directories(
    OmniphonySpatialClosedGateEgressProbe
    PRIVATE "${OMNIPHONY_REALTIME_INCLUDE}"
)
target_link_libraries(OmniphonySpatialClosedGateEgressProbe PRIVATE ole32 uuid avrt)
