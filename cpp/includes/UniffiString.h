/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

#include "Bridging.h"
#include <jsi/jsi.h>

namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<std::string> {
  static jsi::Value arraybuffer_to_string(jsi::Runtime &rt,
                                          const jsi::Value &value) {
    try {
      // value is a Uint8Array, which may be a view into a larger buffer with
      // a non-zero byteOffset. We must read byteOffset and byteLength from the
      // Uint8Array rather than using the full underlying ArrayBuffer.
      auto obj = value.asObject(rt);
      auto buffer = obj.getPropertyAsObject(rt, "buffer").getArrayBuffer(rt);
      auto byteOffset =
          static_cast<size_t>(obj.getProperty(rt, "byteOffset").asNumber());
      auto byteLength =
          static_cast<size_t>(obj.getProperty(rt, "byteLength").asNumber());
      auto string = jsi::String::createFromUtf8(
          rt, buffer.data(rt) + byteOffset, byteLength);
      return jsi::Value(rt, string);
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value string_to_arraybuffer(jsi::Runtime &rt,
                                          const jsi::Value &value) {
    try {
      // Get the string out of the Value.
      auto string = value.asString(rt).utf8(rt);

      // Make a copy of the bytes in the string.
      auto len = string.size();
      auto bytes = new uint8_t[len];
      std::memcpy(bytes, reinterpret_cast<uint8_t *>(string.data()), len);

      // Construct an ArrayBuffer out of the new bytes, then wrap it in a
      // Uint8Array so that byteLength and byteOffset are available to callers.
      auto payload =
          std::make_shared<CMutableBuffer>(CMutableBuffer(bytes, len));
      auto arrayBuffer = jsi::ArrayBuffer(rt, payload);
      auto uint8ArrayCtor =
          rt.global().getPropertyAsFunction(rt, "Uint8Array");
      return uint8ArrayCtor.callAsConstructor(
          rt, jsi::Value(rt, std::move(arrayBuffer)));
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value string_to_bytelength(jsi::Runtime &rt,
                                         const jsi::Value &value) {
    try {
      auto string = value.asString(rt).utf8(rt);
      auto v = static_cast<double>(string.size());
      return jsi::Value(rt, v);
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }
};

} // namespace uniffi_jsi
