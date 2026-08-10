namespace {{ ci.cpp_namespace() }} {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

template <> struct Bridging<RustBuffer> {
  static RustBuffer rustbuffer_alloc(int32_t size) {
      RustCallStatus status = { UNIFFI_CALL_STATUS_OK };
      return {{ ci.ffi_rustbuffer_alloc().name() }}(
          size,
          &status
      );
  }

  static void rustbuffer_free(RustBuffer buf) {
    RustCallStatus status = { UNIFFI_CALL_STATUS_OK };
    {{ ci.ffi_rustbuffer_free().name() }}(
        buf,
        &status
    );
  }

  static RustBuffer rustbuffer_from_bytes(ForeignBytes bytes) {
    RustCallStatus status = { UNIFFI_CALL_STATUS_OK };
    return {{ ci.ffi_rustbuffer_from_bytes().name() }}(
      bytes,
      &status
    );
  }

  static RustBuffer fromJs(jsi::Runtime &rt, std::shared_ptr<CallInvoker>,
                           const jsi::Value &value) {
    try {
      auto obj = value.asObject(rt);

      // Adoption vs copy. Two kinds of view arrive here:
      //
      //   * Library-owned views from `rustbuffer_alloc`. Codegen allocates one,
      //     fills it in place, and ships it as an argument — and never frees a
      //     lowered argument, while the view's backing `CMutableBuffer` is
      //     non-owning (its destructor leaves `data` alone). Such a view carries
      //     a capacity hint (stamped by `rustbuffer_alloc` in the wrapper). We
      //     *adopt* it: hand the existing allocation to the callee, which frees
      //     it. Copying instead would orphan the allocation and leak one whole
      //     payload per call.
      //   * Ordinary JS-owned arrays, which carry no hint. These are *copied*
      //     into a fresh library allocation — they are not ours to give away.
      //
      // On adoption we reset the hint to 0 so a later `rustbuffer_free(view)` is
      // a no-op rather than a double free.
      if (obj.hasProperty(rt, uniffi_jsi::kUbrnRustCapacity)) {
        auto capacity = static_cast<uint64_t>(
            obj.getProperty(rt, uniffi_jsi::kUbrnRustCapacity).asNumber());
        if (capacity == 0) {
          // Hint present but zeroed: the view was already adopted by a previous
          // call and its memory has been freed. Reading it would be a
          // use-after-free, so refuse rather than hand over a dangling pointer.
          throw jsi::JSError(
              rt,
              "RustBuffer argument was already consumed by a previous FFI call");
        }
        auto arrayBuffer =
            obj.getPropertyAsObject(rt, "buffer").getArrayBuffer(rt);
        auto byteOffset =
            static_cast<size_t>(obj.getProperty(rt, "byteOffset").asNumber());
        auto byteLength =
            static_cast<size_t>(obj.getProperty(rt, "byteLength").asNumber());
        obj.setProperty(rt, uniffi_jsi::kUbrnRustCapacity, jsi::Value(0));
        return RustBuffer{
            .capacity = capacity,
            .len = static_cast<uint64_t>(byteLength),
            .data = arrayBuffer.data(rt) + byteOffset,
        };
      }

      auto buffer = uniffi_jsi::Bridging<jsi::ArrayBuffer>::value_to_arraybuffer(rt, value);
      auto bytes = ForeignBytes{
          .len = static_cast<int32_t>(buffer.length(rt)),
          .data = buffer.data(rt),
      };

      // No hint: an ordinary JS-owned array. Rust scaffolding copies the bytes
      // to make the RustBuffer; the copy is destroyed when the callee
      // deserializes its arguments.
      auto buf = rustbuffer_from_bytes(bytes);
      return buf;
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, std::shared_ptr<CallInvoker>,
                         RustBuffer buf) {
    // View-handoff: hand JS a `Uint8Array` view aliasing the Rust-owned bytes
    // (no boundary copy). The single mandatory copy now happens inside
    // `converter.lift(view)` (string decode, byte-array `set`, field-by-field
    // record reads). The codegen-emitted try/finally calls `rustbuffer_free`
    // on the view after `lift` returns, releasing the Rust allocation.
    //
    // Capacity hint: Rust may return a buffer where `capacity > len`. The
    // view's `byteLength` is `len` (so converters that decode the whole view
    // see only the message bytes), but `rustbuffer_free` needs `capacity` to
    // free correctly. We stash `capacity` on the view via a string-keyed
    // property when it differs from `len`; the JSI `rustbufferFree` host
    // function reads it back and falls back to `byteLength` for views from
    // `rustbufferAlloc(n)` where `byteLength == capacity` already.
    //
    // CMutableBuffer is non-owning here: its destructor leaves `buf.data`
    // alone. Only the codegen-emitted `rustbuffer_free` path frees it.
    auto payload = std::make_shared<uniffi_jsi::CMutableBuffer>(
        buf.data, static_cast<size_t>(buf.len));
    auto view = uniffi_jsi::arraybufferToUint8Array(
        rt, jsi::ArrayBuffer(rt, payload));
    if (buf.capacity != static_cast<uint64_t>(buf.len)) {
      view.setProperty(rt, uniffi_jsi::kUbrnRustCapacity,
                       jsi::Value(static_cast<double>(buf.capacity)));
    }
    return jsi::Value(rt, view);
  }
};

} // namespace {{ ci.cpp_namespace() }}
