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

link_libraries(jsi)

add_executable(test-runner test-runner.cpp)
target_link_libraries(test-runner hermes)
