cmake_minimum_required(VERSION 3.20)
project(OmniphonySpatialCompanion LANGUAGES CXX)

if(NOT WIN32)
    message(FATAL_ERROR "The Omniphony spatial companion is Windows-only.")
endif()

add_executable(OmniphonySpatialCompanion
    OmniphonySpatialCompanion.cpp
)

target_compile_features(OmniphonySpatialCompanion PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialCompanion PRIVATE UNICODE _UNICODE)
target_compile_options(OmniphonySpatialCompanion PRIVATE /W4 /permissive- /FIwinstring.h)
target_link_libraries(OmniphonySpatialCompanion PRIVATE windowsapp ole32)

set_property(TARGET OmniphonySpatialCompanion PROPERTY
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
)

add_executable(OmniphonySpatialSetup
    OmniphonySpatialSetup.cpp
    PostSetupSpatialProbe.cpp
)

target_compile_features(OmniphonySpatialSetup PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialSetup PRIVATE UNICODE _UNICODE NOMINMAX)
target_compile_options(OmniphonySpatialSetup PRIVATE /W4 /permissive-)
target_precompile_headers(OmniphonySpatialSetup PRIVATE
    <winrt/Windows.ApplicationModel.h>
    <winrt/Windows.Foundation.Collections.h>
)
target_link_libraries(OmniphonySpatialSetup PRIVATE windowsapp crypt32 shlwapi ole32 advapi32 shell32)
set_target_properties(OmniphonySpatialSetup PROPERTIES
    LINK_FLAGS "/MANIFESTUAC:\"level='asInvoker' uiAccess='false'\""
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
)

add_executable(OmniphonySpatialEndpointProbe
    OmniphonySpatialEndpointProbe.cpp
    ExplorerPause.cpp
)
target_compile_features(OmniphonySpatialEndpointProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialEndpointProbe PRIVATE UNICODE _UNICODE NOMINMAX)
target_compile_options(OmniphonySpatialEndpointProbe PRIVATE /W4 /permissive-)
target_link_libraries(OmniphonySpatialEndpointProbe PRIVATE windowsapp ole32)
set_target_properties(OmniphonySpatialEndpointProbe PROPERTIES
    LINK_FLAGS "/MANIFESTUAC:\"level='asInvoker' uiAccess='false'\""
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
    RUNTIME_OUTPUT_DIRECTORY_RELEASE "${CMAKE_SOURCE_DIR}/../../../dist/Omniphony-Spatial-Companion"
)
