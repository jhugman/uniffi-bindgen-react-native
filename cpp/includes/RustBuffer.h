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

    // Construct an ArrayBuffer.
    auto payload = std::make_shared<CMutableBuffer>(
        CMutableBuffer((uint8_t *)bytes, buf.len));
    auto arrayBuffer = jsi::ArrayBuffer(rt, payload);

    // Once we have a Javascript version, we no longer need the Rust version, so
    // we can call into Rust to tell it it's okay to free that memory.
    rustbuffer_free(buf);

    // Return the ArrayBuffer.
    return jsi::Value(rt, arrayBuffer);
  }
};

} // namespace uniffi_jsi
