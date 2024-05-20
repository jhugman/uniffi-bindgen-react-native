/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

#include "Bridging.h"
#include "ForeignBytes.h"
#include <jsi/jsi.h>

struct RustBuffer {
  size_t capacity;
  size_t len;
  uint8_t *data;
};

RustBuffer rustbuffer_alloc(int32_t size);
void rustbuffer_free(RustBuffer &buf);
RustBuffer rustbuffer_from_bytes(const ForeignBytes &bytes);

namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<RustBuffer> {
  static RustBuffer fromJs(jsi::Runtime &rt, const jsi::Value &value) {
    try {
      auto bytes = uniffi_jsi::Bridging<ForeignBytes>::fromJs(rt, value);
      // This buffer is constructed from foreign bytes. Rust scaffolding copies
      // the bytes, to make the RustBuffer.
      auto buf = rustbuffer_from_bytes(bytes);
      // Once it leaves this function, the buffer is immediately passed back
      // into Rust, where it's used to deserialize into the Rust versions of the
      // arguments. At that point, the copy is destroyed.
      return std::move(buf);
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, RustBuffer buf) {
    // We need to make a copy of the bytes from Rust's memory space into
    // Javascripts memory space. We need to do this because the two languages
    // manages memory very differently: a garbage collector needs to track all
    // the memory at runtime, Rust is doing it all closer to compile time.
    uint8_t *bytes = new uint8_t[buf.len];
    std::memcpy(bytes, buf.data, buf.len);

    // Construct an ArrayBuffer with copy of the bytes from the RustBuffer.
    auto payload = std::make_shared<CMutableBuffer>(
        CMutableBuffer((uint8_t *)bytes, buf.len));
    auto arrayBuffer = jsi::ArrayBuffer(rt, payload);

    // Once we have a Javascript version, we no longer need the Rust version, so
    // we can call into Rust to tell it it's okay to free that memory.
    rustbuffer_free(buf);

    // Finally, return the ArrayBuffer.
    return jsi::Value(rt, arrayBuffer);
  }

  // If we want this to be used, we should make FfiType::requires_cleanup()
  // return true.
  static void argument_cleanup(jsi::Runtime &rt, RustBuffer buf) {
    // NOOP
  }
};

} // namespace uniffi_jsi
