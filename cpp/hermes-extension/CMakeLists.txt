# This is free and unencumbered software released into the public domain.
# For more information, please refer to the LICENSE file in the root directory
# or at <http://unlicense.org/>

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

# Build the rust crate of our choice.
set(RUST_LIB_NAME "" CACHE STRING "Name of the Rust library")
set(RUST_TARGET_DIR "" CACHE PATH "Path to the Rust target dylib directory")
set(HERMES_EXTENSION_NAME "" CACHE STRING "Name of Hermes extension library")
set(HERMES_EXTENSION_CPP "" CACHE PATH "Extension cpp (generated)")

# Add Hermes include directories
include_directories("${HERMES_SRC_DIR}/API/jsi")
include_directories("../includes")

# Add Hermes library directories
link_directories("${HERMES_BUILD_DIR}/jsi")
link_libraries(jsi)

add_library(${HERMES_EXTENSION_NAME} SHARED ${HERMES_EXTENSION_CPP})
