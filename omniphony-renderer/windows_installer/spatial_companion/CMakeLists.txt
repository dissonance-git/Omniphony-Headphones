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

add_executable(OmniphonySpatialNativeProbe
    OmniphonySpatialNativeProbe.cpp
    ExplorerPause.cpp
)
target_compile_features(OmniphonySpatialNativeProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialNativeProbe PRIVATE UNICODE _UNICODE NOMINMAX)
target_compile_options(OmniphonySpatialNativeProbe PRIVATE /W4 /permissive-)
target_link_libraries(OmniphonySpatialNativeProbe PRIVATE windowsapp ole32)
set_target_properties(OmniphonySpatialNativeProbe PROPERTIES
    LINK_FLAGS "/MANIFESTUAC:\"level='asInvoker' uiAccess='false'\""
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
    RUNTIME_OUTPUT_DIRECTORY_RELEASE "${CMAKE_SOURCE_DIR}/../../../dist/Omniphony-Spatial-Companion"
)

add_executable(OmniphonySpatialLicenseEtwProbe
    OmniphonySpatialLicenseEtwProbe.cpp
    ExplorerPause.cpp
)
target_compile_features(OmniphonySpatialLicenseEtwProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialLicenseEtwProbe PRIVATE UNICODE _UNICODE NOMINMAX)
target_compile_options(OmniphonySpatialLicenseEtwProbe PRIVATE /W4 /permissive-)
target_link_libraries(OmniphonySpatialLicenseEtwProbe PRIVATE windowsapp ole32 advapi32 shell32 tdh)
set_target_properties(OmniphonySpatialLicenseEtwProbe PROPERTIES
    LINK_FLAGS "/MANIFESTUAC:\"level='asInvoker' uiAccess='false'\""
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
    RUNTIME_OUTPUT_DIRECTORY_RELEASE "${CMAKE_SOURCE_DIR}/../../../dist/Omniphony-Spatial-Companion"
)

add_executable(OmniphonySpatialLicenseEtlProbe
    OmniphonySpatialLicenseEtlProbe.cpp
    ExplorerPause.cpp
)
target_compile_features(OmniphonySpatialLicenseEtlProbe PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialLicenseEtlProbe PRIVATE UNICODE _UNICODE NOMINMAX)
target_compile_options(OmniphonySpatialLicenseEtlProbe PRIVATE /W4 /permissive-)
target_link_libraries(OmniphonySpatialLicenseEtlProbe PRIVATE windowsapp ole32 advapi32 shell32 tdh)
set_target_properties(OmniphonySpatialLicenseEtlProbe PROPERTIES
    LINK_FLAGS "/MANIFESTUAC:\"level='asInvoker' uiAccess='false'\""
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
    RUNTIME_OUTPUT_DIRECTORY_RELEASE "${CMAKE_SOURCE_DIR}/../../../dist/Omniphony-Spatial-Companion"
)

add_executable(OmniphonySpatialLicenseEtlProbeV13
    OmniphonySpatialLicenseEtlProbeV13.cpp
    ExplorerPause.cpp
)
target_compile_features(OmniphonySpatialLicenseEtlProbeV13 PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialLicenseEtlProbeV13 PRIVATE UNICODE _UNICODE NOMINMAX)
target_compile_options(OmniphonySpatialLicenseEtlProbeV13 PRIVATE /W4 /permissive-)
target_link_libraries(OmniphonySpatialLicenseEtlProbeV13 PRIVATE windowsapp ole32 advapi32 shell32 tdh)
set_target_properties(OmniphonySpatialLicenseEtlProbeV13 PROPERTIES
    LINK_FLAGS "/MANIFESTUAC:\"level='asInvoker' uiAccess='false'\""
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
    RUNTIME_OUTPUT_DIRECTORY_RELEASE "${CMAKE_SOURCE_DIR}/../../../dist/Omniphony-Spatial-Companion"
)

add_executable(OmniphonySpatialLicenseEtlProbeV14
    OmniphonySpatialLicenseEtlProbeV14.cpp
    ExplorerPause.cpp
)
target_compile_features(OmniphonySpatialLicenseEtlProbeV14 PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialLicenseEtlProbeV14 PRIVATE UNICODE _UNICODE NOMINMAX)
target_compile_options(OmniphonySpatialLicenseEtlProbeV14 PRIVATE /W4 /permissive-)
target_link_libraries(OmniphonySpatialLicenseEtlProbeV14 PRIVATE windowsapp ole32 advapi32 shell32 tdh)
set_target_properties(OmniphonySpatialLicenseEtlProbeV14 PROPERTIES
    LINK_FLAGS "/MANIFESTUAC:\"level='asInvoker' uiAccess='false'\""
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
    RUNTIME_OUTPUT_DIRECTORY_RELEASE "${CMAKE_SOURCE_DIR}/../../../dist/Omniphony-Spatial-Companion"
)

add_executable(OmniphonySpatialLicenseEtlProbeV15
    OmniphonySpatialLicenseEtlProbeV15.cpp
    ExplorerPause.cpp
)
target_compile_features(OmniphonySpatialLicenseEtlProbeV15 PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialLicenseEtlProbeV15 PRIVATE UNICODE _UNICODE NOMINMAX)
target_compile_options(OmniphonySpatialLicenseEtlProbeV15 PRIVATE /W4 /permissive-)
target_link_libraries(OmniphonySpatialLicenseEtlProbeV15 PRIVATE windowsapp ole32 advapi32 shell32 tdh)
set_target_properties(OmniphonySpatialLicenseEtlProbeV15 PROPERTIES
    LINK_FLAGS "/MANIFESTUAC:\"level='asInvoker' uiAccess='false'\""
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
    RUNTIME_OUTPUT_DIRECTORY_RELEASE "${CMAKE_SOURCE_DIR}/../../../dist/Omniphony-Spatial-Companion"
)

add_executable(OmniphonySpatialMediaMetadataProbeV16
    OmniphonySpatialMediaMetadataProbeV16.cpp
    ExplorerPause.cpp
)
target_compile_features(OmniphonySpatialMediaMetadataProbeV16 PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialMediaMetadataProbeV16 PRIVATE UNICODE _UNICODE NOMINMAX)
target_compile_options(OmniphonySpatialMediaMetadataProbeV16 PRIVATE /W4 /permissive-)
target_link_libraries(OmniphonySpatialMediaMetadataProbeV16 PRIVATE windowsapp ole32)
set_target_properties(OmniphonySpatialMediaMetadataProbeV16 PROPERTIES
    LINK_FLAGS "/MANIFESTUAC:\"level='asInvoker' uiAccess='false'\""
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
    RUNTIME_OUTPUT_DIRECTORY_RELEASE "${CMAKE_SOURCE_DIR}/../../../dist/Omniphony-Spatial-Companion"
)

add_executable(OmniphonySpatialAppServiceProbeV17
    OmniphonySpatialAppServiceProbeV17.cpp
    ExplorerPause.cpp
)
target_compile_features(OmniphonySpatialAppServiceProbeV17 PRIVATE cxx_std_17)
target_compile_definitions(OmniphonySpatialAppServiceProbeV17 PRIVATE UNICODE _UNICODE NOMINMAX)
target_compile_options(OmniphonySpatialAppServiceProbeV17 PRIVATE /W4 /permissive-)
target_link_libraries(OmniphonySpatialAppServiceProbeV17 PRIVATE windowsapp ole32)
set_target_properties(OmniphonySpatialAppServiceProbeV17 PROPERTIES
    LINK_FLAGS "/MANIFESTUAC:\"level='asInvoker' uiAccess='false'\""
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
)
