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
# Validate HERMES_BUILD_DIR by checking for bin/hermes with the platform-specific extension
if (NOT EXISTS "${HERMES_BUILD_DIR}/bin/hermes${CMAKE_EXECUTABLE_SUFFIX}")
    message(FATAL_ERROR "HERMES_BUILD_DIR does not contain bin/hermes${CMAKE_EXECUTABLE_SUFFIX}")
endif ()

# The Rust cdylib to link against.
set(RUST_CDYLIB "" CACHE FILEPATH "Full path to the Rust cdylib (e.g. libfoo.dylib)")
set(HERMES_EXTENSION_NAME "" CACHE STRING "Name of Hermes extension library")
set(HERMES_EXTENSION_CPP "" CACHE PATH "Extension cpp (generated)")

if (NOT RUST_CDYLIB)
    message(FATAL_ERROR "Please specify RUST_CDYLIB")
endif ()
if (NOT EXISTS "${RUST_CDYLIB}")
    message(FATAL_ERROR "RUST_CDYLIB does not exist: ${RUST_CDYLIB}")
endif ()

# Extract the directory and filename for linking
get_filename_component(RUST_LIB_DIR "${RUST_CDYLIB}" DIRECTORY)
get_filename_component(RUST_LIB_FULLNAME "${RUST_CDYLIB}" NAME)
# Strip lib prefix and .dylib/.so/.dll suffix to get the linker name
string(REGEX REPLACE "^lib" "" RUST_LIB_LINKNAME "${RUST_LIB_FULLNAME}")
string(REGEX REPLACE "\\.(dylib|so|dll)$" "" RUST_LIB_LINKNAME "${RUST_LIB_LINKNAME}")

# Add Hermes include directories
include_directories("${HERMES_SRC_DIR}/API")
include_directories("${HERMES_SRC_DIR}/API/jsi")
include_directories("${HERMES_SRC_DIR}/public")
include_directories("../includes")
include_directories("../stubs")

# Add Hermes library directories
link_directories("${HERMES_BUILD_DIR}/API/hermes")
link_directories("${HERMES_BUILD_DIR}/jsi")
link_libraries(jsi)
link_directories("${RUST_LIB_DIR}")

# Set C++ compiler flags
set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} -O0 -frtti -fexceptions -Wall -fstack-protector-all -Werror")

add_library(${HERMES_EXTENSION_NAME} SHARED ${HERMES_EXTENSION_CPP})
target_link_libraries(${HERMES_EXTENSION_NAME} ${RUST_LIB_LINKNAME})

# Precompile heavy Hermes JSI headers — each .cpp file re-parses these otherwise.
target_precompile_headers(${HERMES_EXTENSION_NAME} PRIVATE
    <jsi/jsi.h>
    <jsi/jsi-inl.h>
)
