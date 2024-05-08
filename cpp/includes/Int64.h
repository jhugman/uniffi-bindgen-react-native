#pragma once

#include "Bridging.h"
#include <jsi/jsi.h>

namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<int64_t> {
  static int64_t fromJs(jsi::Runtime &rt, const jsi::Value &value) {
    try {
      return value.getBigInt(rt).asInt64(rt);
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, int64_t value) {
    auto v = jsi::BigInt::fromInt64(rt, value);
    return jsi::Value(rt, v);
  }
};

} // namespace uniffi_jsi
