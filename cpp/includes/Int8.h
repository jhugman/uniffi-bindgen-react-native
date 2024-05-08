#pragma once

#include "Bridging.h"
#include <jsi/jsi.h>

namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<int8_t> {
  static int8_t fromJs(jsi::Runtime &rt, const jsi::Value &value) {
    try {
      auto v = value.getNumber();
      return static_cast<int8_t>(v);
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, int8_t value) {
    auto v = static_cast<double>(value);
    return jsi::Value(rt, v);
  }
};

} // namespace uniffi_jsi
