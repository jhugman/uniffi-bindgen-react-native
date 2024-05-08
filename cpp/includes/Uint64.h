#pragma once

#include "Bridging.h"
#include <jsi/jsi.h>

namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<uint64_t> {
  static uint64_t fromJs(jsi::Runtime &rt, const jsi::Value &value) {
    try {
      return value.getBigInt(rt).asUint64(rt);
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, uint64_t value) {
    auto v = jsi::BigInt::fromUint64(rt, value);
    return jsi::Value(rt, v);
  }
};

} // namespace uniffi_jsi
