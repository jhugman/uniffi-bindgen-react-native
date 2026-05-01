/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

namespace uniffi_jsi {

// Declare the Bridging template
template <typename T> struct Bridging;

// Property name used to stash a Rust-side capacity hint on a Uint8Array
// view that aliases Rust-owned memory. Set by `Bridging<RustBuffer>::toJs`
// when `capacity != len`; read by the JSI `rustbufferFree` host function
// to free with the original allocation's capacity. Defining this once
// here avoids silent drift between the two sites.
constexpr const char *kUbrnRustCapacity = "__ubrnRustCapacity";

/* ArrayBuffer constructor expects MutableBuffer*/
class CMutableBuffer : public jsi::MutableBuffer {
public:
  CMutableBuffer(uint8_t *data, size_t len) : _data(data), len(len) {}
  size_t size() const override { return len; }
  uint8_t *data() override { return _data; }

private:
  uint8_t *_data;
  size_t len;
};

// Wrap an `ArrayBuffer` as a `Uint8Array` by invoking the JS-side
// `Uint8Array` constructor. Used wherever native code hands a buffer of
// bytes to JS — the wasm/napi runtimes expose `Uint8Array` directly, and
// JSI follows suit so converters can decode against a single shape. The
// constructor lookup is a fast Hermes dictionary read; caching is a
// possible follow-up if profiling shows it's worth the plumbing.
inline jsi::Object arraybufferToUint8Array(jsi::Runtime &rt,
                                           jsi::ArrayBuffer arrayBuffer) {
  auto u8ctor = rt.global().getPropertyAsFunction(rt, "Uint8Array");
  return u8ctor.callAsConstructor(rt, jsi::Value(rt, std::move(arrayBuffer)))
      .asObject(rt);
}

} // namespace uniffi_jsi
