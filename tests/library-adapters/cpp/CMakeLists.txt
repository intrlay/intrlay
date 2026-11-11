cmake_minimum_required(VERSION 3.24)
project(intrlay_glm_adapters LANGUAGES CXX)
include(FetchContent)
# Only the test harness downloads GLM. Installed Intrlay headers do not add it as
# a mandatory package dependency; consumers select their own compatible GLM.
FetchContent_Declare(glm
    URL https://github.com/g-truc/glm/archive/refs/tags/1.0.3.tar.gz
    URL_HASH SHA256=6775e47231a446fd086d660ecc18bcd076531cfedd912fbd66e576b118607001
    SOURCE_SUBDIR intrlay-unused)
FetchContent_MakeAvailable(glm)
file(WRITE "${CMAKE_BINARY_DIR}/glm-source.txt" "${glm_SOURCE_DIR}")

enable_testing()
foreach(mode packed aligned wxyz)
    add_executable(${mode} main.cpp)
    target_compile_features(${mode} PRIVATE cxx_std_20)
    target_include_directories(${mode} PRIVATE ../../../packages/cpp/include "${INTRLAY_FIXTURES}")
    target_include_directories(${mode} SYSTEM PRIVATE "${glm_SOURCE_DIR}")
    if(MSVC)
        target_compile_options(${mode} PRIVATE /W4 /WX /permissive- /Zc:preprocessor)
    else()
        target_compile_options(${mode} PRIVATE -Wall -Wextra -Werror)
    endif()
    if(NOT mode STREQUAL packed)
        target_compile_definitions(${mode} PRIVATE GLM_FORCE_INTRINSICS)
    endif()
    if(mode STREQUAL wxyz)
        target_compile_definitions(${mode} PRIVATE GLM_FORCE_QUAT_DATA_WXYZ)
    endif()
    add_test(NAME ${mode} COMMAND ${mode})
endforeach()
