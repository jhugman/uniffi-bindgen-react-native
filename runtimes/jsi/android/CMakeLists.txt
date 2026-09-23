#
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/
#

cmake_minimum_required(VERSION 3.13)
project(ubrn_jsi_player)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_VERBOSE_MAKEFILE ON)

add_library(ubrn_jsi_player SHARED
  ../cpp/shim.cpp
  ../cpp/callbacks.cpp
  ../cpp/abi_assert.cpp
  cpp-adapter.cpp
)
target_include_directories(ubrn_jsi_player PRIVATE ../cpp ../include)
target_compile_options(ubrn_jsi_player PRIVATE -O2 -fexceptions -frtti -fstack-protector-all -Wall -Werror)
# Android 15+ refuses libraries that are not 16 KB page-aligned.
target_link_options(ubrn_jsi_player PRIVATE -Wl,-z,max-page-size=16384)

# The Rust half: core dispatch, libffi and dlopen, prebuilt per ABI as a
# shared library. Gradle packages it from jniLibs; the system linker resolves
# it from the app's native-library directory when this library loads.
add_library(uniffi_runtime_jsi SHARED IMPORTED)
set_target_properties(uniffi_runtime_jsi PROPERTIES
  IMPORTED_LOCATION ${CMAKE_CURRENT_SOURCE_DIR}/../prebuilt/android/${ANDROID_ABI}/libuniffi_runtime_jsi.so)

find_package(ReactAndroid REQUIRED CONFIG)
find_package(fbjni REQUIRED CONFIG)
find_library(LOGCAT log)

target_link_libraries(ubrn_jsi_player
  ReactAndroid::reactnative
  ReactAndroid::jsi
  fbjni::fbjni
  ${LOGCAT}
  uniffi_runtime_jsi
)
