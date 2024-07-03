/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

#include "Bridging.h"
#include <jsi/jsi.h>

template <typename T> struct ReferenceHolder {
  T pointee;
};

namespace uniffi_jsi {
using namespace facebook;

template <typename T> struct Bridging<ReferenceHolder<T>> {
  static jsi::Value jsNew(jsi::Runtime &rt) {
    auto holder = jsi::Object(rt);
    return holder;
  }
  static T fromJs(jsi::Runtime &rt, const jsi::Value &value) {
    auto obj = value.asObject(rt);
    if (obj.hasProperty(rt, "pointee")) {
      auto pointee = obj.getProperty(rt, "pointee");
      return uniffi_jsi::Bridging<T>::fromJs(rt, pointee);
    }
  }
};

} // namespace uniffi_jsi
