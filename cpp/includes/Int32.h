/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

#include "Bridging.h"
#include "UniffiCallInvoker.h"
#include <jsi/jsi.h>

namespace uniffi_jsi {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

template <> struct Bridging<int32_t> {
  static int32_t fromJs(jsi::Runtime &rt, std::shared_ptr<CallInvoker>,
                        const jsi::Value &value) {
    try {
      auto v = value.getNumber();
      return static_cast<int32_t>(v);
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, std::shared_ptr<CallInvoker>,
                         int32_t value) {
    auto v = static_cast<double>(value);
    return jsi::Value(rt, v);
  }
};

} // namespace uniffi_jsi
