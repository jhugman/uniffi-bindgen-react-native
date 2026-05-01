#
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/
#

cmake_minimum_required(VERSION 3.22)
project(test-runner)

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

# Add Hermes include directories
include_directories("${HERMES_SRC_DIR}/API")
include_directories("${HERMES_SRC_DIR}/API/jsi")
include_directories("${HERMES_SRC_DIR}/public")
include_directories("../includes")
include_directories("../stubs")

# Add Hermes library directories
link_directories("${HERMES_BUILD_DIR}/API/hermes${HERMES_CONFIG_SUBDIR}")
link_directories("${HERMES_BUILD_DIR}/jsi${HERMES_CONFIG_SUBDIR}")
link_directories("${HERMES_BUILD_DIR}/lib${HERMES_CONFIG_SUBDIR}")

link_libraries(jsi)

# Detect hermes library name and location, which changed in newer Hermes branches:
# - stable branches (e.g. rn/0.77-stable): 'hermes' in API/hermes/ (full VM + compiler)
# - main branch: 'hermesvm' in lib/ (full VM + compiler); 'hermes_lean' in API/hermes/ (no compiler)
# We must use a full-compiler build so the test-runner can evaluate JS source directly.
# find_library handles platform naming differences (lib prefix, .so/.dylib/.lib suffixes).
set(HERMES_LIB_SEARCH_PATHS
    "${HERMES_BUILD_DIR}/API/hermes${HERMES_CONFIG_SUBDIR}"
    "${HERMES_BUILD_DIR}/lib${HERMES_CONFIG_SUBDIR}"
)
find_library(HERMES_LIB NAMES hermes hermesvm PATHS ${HERMES_LIB_SEARCH_PATHS} NO_DEFAULT_PATH)
if (NOT HERMES_LIB)
    message(FATAL_ERROR "Could not find hermes or hermesvm library in ${HERMES_BUILD_DIR}")
endif ()
get_filename_component(HERMES_LIB_NAME "${HERMES_LIB}" NAME_WE)
# Strip the "lib" prefix if present (Unix), so we get the bare linker name.
string(REGEX REPLACE "^lib" "" HERMES_LIB_NAME "${HERMES_LIB_NAME}")

add_executable(test-runner test-runner.cpp)
# Prevent windows.h from defining min/max macros that conflict with Hermes headers.
if (MSVC)
    target_compile_definitions(test-runner PRIVATE NOMINMAX)
endif ()
target_link_libraries(test-runner ${HERMES_LIB_NAME})
