/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

#include "Bridging.h"
#include <jsi/jsi.h>

struct ForeignBytes {
  int32_t len;
  uint8_t *data;
};

// This isn't needed if the bytes have been used to _make_ a RustBuffer.
void foreign_bytes_free(ForeignBytes *fb) {
  fb->data = nullptr;
  delete fb;
}

namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<ForeignBytes> {
  static ForeignBytes fromJs(jsi::Runtime &rt, const jsi::Value &value) {
    try {
      auto buffer = value.asObject(rt).getArrayBuffer(rt);
      auto bytes = ForeignBytes{
          .len = static_cast<int32_t>(buffer.length(rt)),
          .data = buffer.data(rt),
      };
      return std::move(bytes);
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, std::shared_ptr<CallInvoker>,
                         ForeignBytes value) {
    throw jsi::JSError(rt, "Unreachable ForeignBytes.toJs");
  }
};

} // namespace uniffi_jsi
