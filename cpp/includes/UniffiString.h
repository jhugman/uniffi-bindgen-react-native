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
  static jsi::Value string_from_buffer(jsi::Runtime &rt,
                                       const jsi::Value &value) {
    try {
      auto obj = value.asObject(rt);
      // Handle both Uint8Array views (with byteOffset/byteLength) and
      // plain {buffer: ArrayBuffer} wrappers.
      if (obj.hasProperty(rt, "byteOffset")) {
        // TypedArray (e.g. Uint8Array view) — respect offset and length.
        auto buffer = obj.getPropertyAsObject(rt, "buffer").getArrayBuffer(rt);
        auto byteOffset =
            static_cast<size_t>(obj.getProperty(rt, "byteOffset").asNumber());
        auto byteLength =
            static_cast<size_t>(obj.getProperty(rt, "byteLength").asNumber());
        auto string = jsi::String::createFromUtf8(
            rt, buffer.data(rt) + byteOffset, byteLength);
        return jsi::Value(rt, string);
      }
      auto buffer =
          uniffi_jsi::Bridging<jsi::ArrayBuffer>::value_to_arraybuffer(rt,
                                                                       value);
      auto string =
          jsi::String::createFromUtf8(rt, buffer.data(rt), buffer.length(rt));
      return jsi::Value(rt, string);
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value string_to_buffer(jsi::Runtime &rt,
                                     const jsi::Value &value) {
    try {
      // Get the string out of the Value.
      auto string = value.asString(rt).utf8(rt);

      // Make a copy of the bytes in the string.
      auto len = string.size();
      auto bytes = new uint8_t[len];
      std::memcpy(bytes, reinterpret_cast<uint8_t *>(string.data()), len);

      // Construct an array buffer out of the new bytes.
      auto payload =
          std::make_shared<CMutableBuffer>(CMutableBuffer(bytes, len));
      auto arrayBuffer = jsi::ArrayBuffer(rt, payload);

      return uniffi_jsi::Bridging<jsi::ArrayBuffer>::arraybuffer_to_value(
          rt, arrayBuffer);
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

  /// Read a string directly from an ArrayBuffer at the given byte offset and
  /// length. No JS-side Uint8Array view allocation, no property lookups for
  /// byteOffset/byteLength — significantly faster than
  /// string_from_buffer(view) for array-of-strings workloads.
  ///
  ///   args[0]: source object (with .arrayBuffer or .buffer property)
  ///   args[1]: byte offset (number)
  ///   args[2]: byte length (number)
  static jsi::Value read_string_from_buffer(jsi::Runtime &rt,
                                            const jsi::Value &bufValue,
                                            const jsi::Value &offsetValue,
                                            const jsi::Value &lengthValue) {
    try {
      auto obj = bufValue.asObject(rt);
      jsi::ArrayBuffer buffer =
          obj.hasProperty(rt, "arrayBuffer")
              ? obj.getPropertyAsObject(rt, "arrayBuffer").getArrayBuffer(rt)
              : obj.getPropertyAsObject(rt, "buffer").getArrayBuffer(rt);
      auto offset = static_cast<size_t>(offsetValue.asNumber());
      auto length = static_cast<size_t>(lengthValue.asNumber());
      auto string =
          jsi::String::createFromUtf8(rt, buffer.data(rt) + offset, length);
      return jsi::Value(rt, string);
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }
};

} // namespace uniffi_jsi
