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
target_compile_options(OmniphonySpatialCompanion PRIVATE /W4 /permissive-)
target_link_libraries(OmniphonySpatialCompanion PRIVATE windowsapp)

set_property(TARGET OmniphonySpatialCompanion PROPERTY
    MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>"
)
