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

struct ForeignBytes {
  int32_t len;
  const uint8_t *data;
};

namespace uniffi_jsi {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

class BorrowedForeignBytes {
public:
  BorrowedForeignBytes(jsi::Runtime &rt, jsi::ArrayBuffer buffer, size_t offset,
                       int32_t length)
      : rt_(rt), buffer_(std::move(buffer)), offset_(offset), length_(length) {}

  operator ForeignBytes() const {
    auto data = buffer_.data(rt_);
    if (length_ > 0 && data == nullptr) {
      throw jsi::JSError(rt_, "ForeignBytes buffer is detached");
    }
    return {length_, data == nullptr ? nullptr : data + offset_};
  }

private:
  jsi::Runtime &rt_;
  jsi::ArrayBuffer buffer_;
  size_t offset_;
  int32_t length_;
};

template <> struct Bridging<ForeignBytes> {
  static BorrowedForeignBytes fromJs(jsi::Runtime &rt,
                                     std::shared_ptr<CallInvoker>,
                                     const jsi::Value &value) {
    auto object = value.asObject(rt);
    auto arrayBufferCtor = rt.global().getPropertyAsObject(rt, "ArrayBuffer");
    auto isView = arrayBufferCtor.getPropertyAsFunction(rt, "isView");
    auto uint8Ctor = rt.global().getPropertyAsFunction(rt, "Uint8Array");
    if (!isView.call(rt, value).getBool() || !object.instanceOf(rt, uint8Ctor)) {
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
    if (!bufferObject.isArrayBuffer(rt)) {
      throw jsi::JSError(rt, "ForeignBytes requires a non-shared ArrayBuffer");
    }
    auto buffer = bufferObject.getArrayBuffer(rt);
    auto size = buffer.size(rt);
    if (offset > static_cast<double>(size) ||
        length > static_cast<double>(size) - offset) {
      throw jsi::JSError(rt, "ForeignBytes view is outside its ArrayBuffer");
    }
    return BorrowedForeignBytes(rt, std::move(buffer),
                                static_cast<size_t>(offset),
                                static_cast<int32_t>(length));
  }
};
} // namespace uniffi_jsi
