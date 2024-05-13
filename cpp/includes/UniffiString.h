#pragma once

#include "Bridging.h"
#include <jsi/jsi.h>

namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<std::string> {
  static jsi::Value arraybuffer_to_string(jsi::Runtime &rt,
                                          const jsi::Value &value) {
    try {
      auto buffer = value.asObject(rt).getArrayBuffer(rt);
      auto string =
          jsi::String::createFromUtf8(rt, buffer.data(rt), buffer.length(rt));
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

      // Construct an array buffer out of the new bytes.
      auto payload =
          std::make_shared<CMutableBuffer>(CMutableBuffer(bytes, len));
      auto arrayBuffer = jsi::ArrayBuffer(rt, payload);

      return jsi::Value(rt, arrayBuffer);
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
