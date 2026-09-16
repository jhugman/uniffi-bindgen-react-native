/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

#include "Bridging.h"
#include "UniffiCallInvoker.h"
#include <cmath>
#include <jsi/jsi.h>
#include <limits>
#include <utility>

struct ForeignBytes {
  int32_t len;
  const uint8_t *data;
};

namespace uniffi_jsi {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

/// A `ForeignBytes` that borrows a JS `Uint8Array`'s storage.
///
/// Valid only until the next JS execution on the runtime; must not be stored.
class BorrowedForeignBytes {
public:
  BorrowedForeignBytes(int32_t length, size_t offset, jsi::ArrayBuffer buffer)
      : length_{length}, offset_{offset}, buffer_{std::move(buffer)} {}

  /// Read the storage pointer. Runs no JS; throws if `buffer_` was detached.
  void capture(jsi::Runtime &rt) {
    auto *data = buffer_.data(rt);
    if (length_ > 0 && data == nullptr) {
      throw jsi::JSError(rt, "ForeignBytes buffer is detached");
    }
    bytes_ = ForeignBytes{
        length_,
        data == nullptr ? nullptr : data + offset_,
    };
  }

  operator ForeignBytes() const { return bytes_; }

private:
  int32_t length_;
  size_t offset_;
  jsi::ArrayBuffer buffer_;
  ForeignBytes bytes_{0, nullptr};
};

template <> struct Bridging<ForeignBytes> {
  static BorrowedForeignBytes fromJs(jsi::Runtime &rt,
                                     std::shared_ptr<CallInvoker>,
                                     const jsi::Value &value) {
    auto object = value.asObject(rt);
    auto arrayBufferCtor = rt.global().getPropertyAsObject(rt, "ArrayBuffer");
    auto isView = arrayBufferCtor.getPropertyAsFunction(rt, "isView");
    auto uint8Ctor = rt.global().getPropertyAsFunction(rt, "Uint8Array");
    if (!isView.call(rt, value).getBool() ||
        !object.instanceOf(rt, uint8Ctor)) {
      throw jsi::JSError(rt, "ForeignBytes requires a Uint8Array");
    }
    auto bufferObject = object.getPropertyAsObject(rt, "buffer");
    auto offset = object.getProperty(rt, "byteOffset").asNumber();
    auto length = object.getProperty(rt, "byteLength").asNumber();
    if (!std::isfinite(offset) || offset < 0 || std::floor(offset) != offset ||
        !std::isfinite(length) || length < 0 || std::floor(length) != length ||
        length > std::numeric_limits<int32_t>::max()) {
      throw jsi::JSError(rt, "Invalid ForeignBytes offset or length");
    }
    // A capacity hint present and zeroed marks a view whose allocation was
    // adopted and freed by a previous call; borrowing it would be a
    // use-after-free. Borrowing does not reset the hint.
    if (object.hasProperty(rt, kUbrnRustCapacity) &&
        object.getProperty(rt, kUbrnRustCapacity).asNumber() == 0) {
      throw jsi::JSError(
          rt, "ForeignBytes argument was already consumed by a previous FFI call");
    }
    if (!bufferObject.isArrayBuffer(rt)) {
      throw jsi::JSError(rt, "ForeignBytes requires a non-shared ArrayBuffer");
    }
    auto buffer = bufferObject.getArrayBuffer(rt);
    auto size = buffer.size(rt);
    if (offset > static_cast<double>(size) ||
        length > static_cast<double>(size) - offset) {
      throw jsi::JSError(rt, "ForeignBytes view is outside its ArrayBuffer");
    }
    return BorrowedForeignBytes(static_cast<int32_t>(length),
                                static_cast<size_t>(offset),
                                std::move(buffer));
  }
};
} // namespace uniffi_jsi
