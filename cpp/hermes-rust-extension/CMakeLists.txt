#
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/
#

cmake_minimum_required(VERSION 3.22)
project(${HERMES_EXTENSION_NAME})

set(CMAKE_CXX_STANDARD 17)

set(HERMES_SRC_DIR "" CACHE PATH "Path to Hermes source directory")
set(HERMES_BUILD_DIR "" CACHE PATH "Path to Hermes build directory")

if (NOT HERMES_SRC_DIR)
    message(FATAL_ERROR "Please specify HERMES_SRC_DIR")
endif ()
# Validate HERMES_SRC_DIR by checking for API/jsi/jsi/jsi.h
if (NOT EXISTS "${HERMES_SRC_DIR}/API/jsi/jsi/jsi.h")
    message(FATAL_ERROR "HERMES_SRC_DIR does not contain API/jsi/jsi/jsi.h")
endif ()

if (NOT HERMES_BUILD_DIR)
    message(FATAL_ERROR "Please specify HERMES_BUILD_DIR")
endif ()
# On multi-config generators (e.g. Visual Studio), build artifacts are placed
# under a per-config subdirectory (e.g. Debug/).
if (CMAKE_CONFIGURATION_TYPES)
    set(HERMES_CONFIG_SUBDIR "/Debug")
else ()
    set(HERMES_CONFIG_SUBDIR "")
endif ()

# Validate HERMES_BUILD_DIR by checking for bin/hermes with the platform-specific extension
if (NOT EXISTS "${HERMES_BUILD_DIR}/bin${HERMES_CONFIG_SUBDIR}/hermes${CMAKE_EXECUTABLE_SUFFIX}")
    message(FATAL_ERROR "HERMES_BUILD_DIR does not contain bin${HERMES_CONFIG_SUBDIR}/hermes${CMAKE_EXECUTABLE_SUFFIX}")
endif ()

# Build the rust crate of our choice.
set(RUST_LIB_NAME "" CACHE STRING "Name of the Rust library")
set(RUST_TARGET_DIR "" CACHE PATH "Path to the Rust target dylib directory")
set(HERMES_EXTENSION_NAME "" CACHE STRING "Name of Hermes extension library")
set(HERMES_EXTENSION_CPP "" CACHE PATH "Extension cpp (generated)")

if (NOT RUST_LIB_NAME)
    message(FATAL_ERROR "Please specify RUST_LIB_NAME")
endif ()

# Validate RUST_TARGET_DIR by checking for the Rust library
find_library(RUST_LIB_PATH NAMES ${RUST_LIB_NAME} PATHS "${RUST_TARGET_DIR}" NO_DEFAULT_PATH)
if (NOT RUST_LIB_PATH)
    message(FATAL_ERROR "RUST_TARGET_DIR does not contain ${RUST_LIB_NAME} shared library")
endif ()

# Add Hermes include directories
include_directories("${HERMES_SRC_DIR}/API")
include_directories("${HERMES_SRC_DIR}/API/jsi")
include_directories("${HERMES_SRC_DIR}/public")
include_directories("../includes")
include_directories("../stubs")

# Add Hermes library directories
link_directories("${HERMES_BUILD_DIR}/API/hermes${HERMES_CONFIG_SUBDIR}")
link_directories("${HERMES_BUILD_DIR}/jsi${HERMES_CONFIG_SUBDIR}")
link_libraries(jsi)
link_directories("${RUST_TARGET_DIR}")

# Set C++ compiler flags
if (MSVC)
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} /O2 /EHsc /W3 /WX /GS")
else ()
    set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -O2 -frtti -fexceptions -Wall -fstack-protector-all -Werror")
endif ()

add_library(${HERMES_EXTENSION_NAME} SHARED ${HERMES_EXTENSION_CPP})
# Prevent windows.h from defining min/max macros that conflict with Hermes headers.
if (MSVC)
    target_compile_definitions(${HERMES_EXTENSION_NAME} PRIVATE NOMINMAX)
endif ()
target_link_libraries(${HERMES_EXTENSION_NAME} ${RUST_LIB_NAME})
