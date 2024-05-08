#pragma once

#include "Bridging.h"
#include <jsi/jsi.h>

namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<double> {
  static double fromJs(jsi::Runtime &rt, const jsi::Value &value) {
    try {
      return value.getNumber();
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, double value) {
    return jsi::Value(rt, value);
  }
};

} // namespace uniffi_jsi
