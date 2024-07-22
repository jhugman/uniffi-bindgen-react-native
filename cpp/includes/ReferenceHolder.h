/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

#include "Bridging.h"
#include "UniffiCallInvoker.h"
#include <jsi/jsi.h>

template <typename T> struct ReferenceHolder {
  T pointee;
};

namespace uniffi_jsi {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

template <typename T> struct Bridging<ReferenceHolder<T>> {
  static jsi::Value jsNew(jsi::Runtime &rt) {
    auto holder = jsi::Object(rt);
    return holder;
  }
  static T fromJs(jsi::Runtime &rt, std::shared_ptr<CallInvoker> callInvoker,
                  const jsi::Value &value) {
    auto obj = value.asObject(rt);
    if (obj.hasProperty(rt, "pointee")) {
      auto pointee = obj.getProperty(rt, "pointee");
      return uniffi_jsi::Bridging<T>::fromJs(rt, callInvoker, pointee);
    }
    throw jsi::JSError(rt,
                       "Expected ReferenceHolder to have a pointee property. "
                       "This is likely a bug in uniffi-bindgen-react-native");
  }
};

} // namespace uniffi_jsi
