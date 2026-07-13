#
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/
#

cmake_minimum_required(VERSION 3.22)
project(ubrn-jsi-player)

set(CMAKE_CXX_STANDARD 17)

set(HERMES_SRC_DIR "" CACHE PATH "Path to Hermes source directory")
set(HERMES_BUILD_DIR "" CACHE PATH "Path to Hermes build directory")
# Directory containing libuniffi_runtime_jsi.a (target/debug).
set(RUST_TARGET_DIR "" CACHE PATH "Path to the Rust target dir with the jsi staticlib")
# Directory containing ubrn_jsi.h (runtimes/jsi/include).
set(UBRN_JSI_INCLUDE_DIR "" CACHE PATH "Path to runtimes/jsi/include")

if (NOT HERMES_SRC_DIR OR NOT EXISTS "${HERMES_SRC_DIR}/API/jsi/jsi/jsi.h")
    message(FATAL_ERROR "HERMES_SRC_DIR must contain API/jsi/jsi/jsi.h")
endif ()

if (CMAKE_CONFIGURATION_TYPES)
    set(HERMES_CONFIG_SUBDIR "/Debug")
else ()
    set(HERMES_CONFIG_SUBDIR "")
endif ()

include_directories("${HERMES_SRC_DIR}/API")
include_directories("${HERMES_SRC_DIR}/API/jsi")
include_directories("${HERMES_SRC_DIR}/public")
# ../stubs supplies ReactCommon/CallInvoker.h for host builds. The player needs
# nothing from ../includes: those headers are the gen_cpp per-library bridge, which
# the player exists to replace.
include_directories("../stubs")
include_directories("${UBRN_JSI_INCLUDE_DIR}")

link_directories("${HERMES_BUILD_DIR}/jsi${HERMES_CONFIG_SUBDIR}")
link_libraries(jsi)

find_library(UBRN_JSI_LIB NAMES uniffi_runtime_jsi PATHS "${RUST_TARGET_DIR}" NO_DEFAULT_PATH)
if (NOT UBRN_JSI_LIB)
    message(FATAL_ERROR "Could not find uniffi_runtime_jsi staticlib in ${RUST_TARGET_DIR}")
endif ()

set(CMAKE_WINDOWS_EXPORT_ALL_SYMBOLS ON)
add_library(ubrn_jsi_player SHARED shim.cpp callbacks.cpp abi_assert.cpp)
if (MSVC)
    target_compile_definitions(ubrn_jsi_player PRIVATE NOMINMAX)
endif ()
target_link_libraries(ubrn_jsi_player ${UBRN_JSI_LIB})
