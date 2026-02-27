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
# Validate HERMES_BUILD_DIR by checking for bin/hermes with the platform-specific extension
if (NOT EXISTS "${HERMES_BUILD_DIR}/bin/hermes${CMAKE_EXECUTABLE_SUFFIX}")
    message(FATAL_ERROR "HERMES_BUILD_DIR does not contain bin/hermes${CMAKE_EXECUTABLE_SUFFIX}")
endif ()

# Add Hermes include directories
include_directories("${HERMES_SRC_DIR}/API")
include_directories("${HERMES_SRC_DIR}/API/jsi")
include_directories("${HERMES_SRC_DIR}/public")
include_directories("../includes")
include_directories("../stubs")

# Add Hermes library directories
link_directories("${HERMES_BUILD_DIR}/API/hermes")
link_directories("${HERMES_BUILD_DIR}/jsi")
link_directories("${HERMES_BUILD_DIR}/lib")

link_libraries(jsi)

# Detect hermes library name and location, which changed in newer Hermes branches:
# - stable branches (e.g. rn/0.77-stable): 'hermes' in API/hermes/ (full VM + compiler)
# - main branch: 'hermesvm' in lib/ (full VM + compiler); 'hermes_lean' in API/hermes/ (no compiler)
# We must use a full-compiler build so the test-runner can evaluate JS source directly.
if(EXISTS "${HERMES_BUILD_DIR}/API/hermes/libhermes${CMAKE_SHARED_LIBRARY_SUFFIX}")
    set(HERMES_LIB_NAME hermes)
elseif(EXISTS "${HERMES_BUILD_DIR}/lib/libhermesvm${CMAKE_SHARED_LIBRARY_SUFFIX}")
    set(HERMES_LIB_NAME hermesvm)
else()
    message(FATAL_ERROR "Could not find hermes (stable) or hermesvm (main) library in ${HERMES_BUILD_DIR}")
endif()

add_executable(test-runner test-runner.cpp)
target_link_libraries(test-runner ${HERMES_LIB_NAME})
