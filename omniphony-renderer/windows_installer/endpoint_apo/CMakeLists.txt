cmake_minimum_required(VERSION 3.20)
cmake_policy(SET CMP0091 NEW)
project(OmniphonyEndpointApo LANGUAGES CXX)

if(NOT WIN32)
    message(FATAL_ERROR "The Omniphony endpoint APO is Windows-only.")
endif()

# Keep the shipped APO/helpers independent of the Visual C++ redistributable.
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
set(OMNIPHONY_REALTIME_INCLUDE "${CMAKE_CURRENT_SOURCE_DIR}/../../realtime_ffi/include")

# Proven post-mix stereo endpoint APO. Retained as a rollback floor while the
# pre-mix stream APO earns field confidence on real Windows endpoints.
add_library(OmniphonyAPO SHARED
    OmniphonyAPO.cpp
    OmniphonyAPO.def
)
target_compile_features(OmniphonyAPO PRIVATE cxx_std_17)
target_compile_definitions(OmniphonyAPO PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonyAPO PRIVATE /W4 /WX /permissive-)
target_include_directories(OmniphonyAPO PRIVATE "${OMNIPHONY_REALTIME_INCLUDE}")
target_link_libraries(OmniphonyAPO PRIVATE ole32 uuid advapi32 audioeng AudioBaseProcessingObjectV140 legacy_stdio_definitions audiomediatypecrt)
target_link_options(OmniphonyAPO PRIVATE /MANIFEST:NO)
set_target_properties(OmniphonyAPO PROPERTIES PREFIX "" OUTPUT_NAME "OmniphonyAPO")

# Pre-mix SFX. Stereo streams use the proven Current PCM ABI; authored Windows
# multichannel beds use the native-bed ABI and emerge as binaural stereo before
# the endpoint mixer. Samples-per-frame is intentionally allowed to differ.
add_library(OmniphonyStreamAPO SHARED
    OmniphonyStreamAPO.cpp
    OmniphonyStreamAPO.def
)
target_compile_features(OmniphonyStreamAPO PRIVATE cxx_std_17)
target_compile_definitions(OmniphonyStreamAPO PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonyStreamAPO PRIVATE /W4 /WX /permissive-)
target_include_directories(OmniphonyStreamAPO PRIVATE "${OMNIPHONY_REALTIME_INCLUDE}")
target_link_libraries(OmniphonyStreamAPO PRIVATE ole32 uuid advapi32 audioeng AudioBaseProcessingObjectV140 legacy_stdio_definitions audiomediatypecrt windowsapp)
target_link_options(OmniphonyStreamAPO PRIVATE /MANIFEST:NO)
set_target_properties(OmniphonyStreamAPO PROPERTIES PREFIX "" OUTPUT_NAME "OmniphonyStreamAPO")

# Single authoritative endpoint-FX control helper. Installer command dispatch,
# ACL fallback, and read-back verification must be compiled from this one source.
add_executable(OmniphonyApoCtl OmniphonyApoCtl.cpp)
target_compile_features(OmniphonyApoCtl PRIVATE cxx_std_17)
target_compile_definitions(OmniphonyApoCtl PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonyApoCtl PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonyApoCtl PRIVATE ole32 uuid propsys advapi32)

add_executable(OmniphonyApoSmoke OmniphonyApoSmoke.cpp)
target_compile_features(OmniphonyApoSmoke PRIVATE cxx_std_17)
target_compile_definitions(OmniphonyApoSmoke PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonyApoSmoke PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonyApoSmoke PRIVATE ole32 uuid audioeng audiomediatypecrt)

add_executable(OmniphonyStreamApoSmoke OmniphonyStreamApoSmoke.cpp)
target_compile_features(OmniphonyStreamApoSmoke PRIVATE cxx_std_17)
target_compile_definitions(OmniphonyStreamApoSmoke PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonyStreamApoSmoke PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonyStreamApoSmoke PRIVATE ole32 uuid audioeng audiomediatypecrt)

add_executable(OmniphonyStreamFallbackSmoke OmniphonyStreamFallbackSmoke.cpp)
target_compile_features(OmniphonyStreamFallbackSmoke PRIVATE cxx_std_17)
target_compile_definitions(OmniphonyStreamFallbackSmoke PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonyStreamFallbackSmoke PRIVATE /W4 /WX /permissive-)

# RAW processing mode is the single-render escape hatch for future provider
# egress. This smoke runs in its own process and proves both Omniphony APOs stay
# bit-transparent, add zero latency, and never load omniphony_realtime.dll.
add_executable(OmniphonyRawApoSmoke OmniphonyRawApoSmoke.cpp)
target_compile_features(OmniphonyRawApoSmoke PRIVATE cxx_std_17)
target_compile_definitions(OmniphonyRawApoSmoke PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonyRawApoSmoke PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonyRawApoSmoke PRIVATE ole32 uuid audioeng audiomediatypecrt)

add_executable(OmniphonyRealtimeSmoke OmniphonyRealtimeSmoke.cpp)
target_compile_features(OmniphonyRealtimeSmoke PRIVATE cxx_std_17)
target_compile_definitions(OmniphonyRealtimeSmoke PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonyRealtimeSmoke PRIVATE /W4 /WX /permissive-)
target_include_directories(OmniphonyRealtimeSmoke PRIVATE "${OMNIPHONY_REALTIME_INCLUDE}")

add_executable(OmniphonyMixProbe OmniphonyMixProbe.cpp)
target_compile_features(OmniphonyMixProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonyMixProbe PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonyMixProbe PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonyMixProbe PRIVATE ole32 uuid propsys advapi32)

# Production acceptance deliberately uses a separate, read-only WASAPI probe.
# Unlike the development MixProbe, it owns no registry/ACL repair path.
add_executable(OmniphonyProductionProbe OmniphonyProductionProbe.cpp)
target_compile_features(OmniphonyProductionProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonyProductionProbe PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonyProductionProbe PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonyProductionProbe PRIVATE ole32 uuid)

# Read-only native Spatial Audio capability probe. It interrogates the active
# endpoint's ISpatialAudioClient without opening a render stream or claiming
# access to another process's spatial objects.
add_executable(OmniphonySpatialProbe OmniphonySpatialProbe.cpp)
target_compile_features(OmniphonySpatialProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialProbe PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonySpatialProbe PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonySpatialProbe PRIVATE ole32 uuid)

# Read-only observation probe for the undocumented Windows spatial-provider
# registry surfaces. Its optional --probe-com canary asks only whether an
# observed encoder CLSID directly exposes the public ISpatialAudioClient COM
# interface. It never changes provider selection, opens a render stream,
# registers/unregisters anything, repairs ACLs, or takes ownership.
add_executable(OmniphonySpatialProviderProbe OmniphonySpatialProviderProbe.cpp)
target_compile_features(OmniphonySpatialProviderProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialProviderProbe PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonySpatialProviderProbe PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonySpatialProviderProbe PRIVATE ole32 uuid advapi32)

# Provider-egress gate. This never starts playback. It asks whether the active
# physical endpoint advertises RAW processing and whether a shared stereo
# IAudioClient2 can be initialized with AUDCLNT_STREAMOPTIONS_RAW. A future
# spatial provider uses this seam to bypass the normal Omniphony stream SFX and
# prevent a second binaural render.
add_executable(OmniphonyRawProbe OmniphonyRawProbe.cpp)
target_compile_features(OmniphonyRawProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonyRawProbe PRIVATE UNICODE _UNICODE WIN32_LEAN_AND_MEAN NOMINMAX)
target_compile_options(OmniphonyRawProbe PRIVATE /W4 /WX /permissive-)
target_link_libraries(OmniphonyRawProbe PRIVATE ole32 uuid propsys)
