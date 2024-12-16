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

template <> struct Bridging<jsi::ArrayBuffer> {
  static jsi::ArrayBuffer value_to_arraybuffer(jsi::Runtime &rt,
                                               const jsi::Value &value) {
    try {
      return value.asObject(rt)
          .getPropertyAsObject(rt, "buffer")
          .getArrayBuffer(rt);
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value arraybuffer_to_value(jsi::Runtime &rt,
                                         const jsi::ArrayBuffer &arrayBuffer) {
    try {
      jsi::Object obj(rt);
      obj.setProperty(rt, "buffer", arrayBuffer);
      return jsi::Value(rt, obj);
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }
};

} // namespace uniffi_jsi
