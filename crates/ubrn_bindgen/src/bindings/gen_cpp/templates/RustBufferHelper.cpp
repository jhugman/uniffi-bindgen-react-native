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
      auto buffer = uniffi_jsi::Bridging<jsi::ArrayBuffer>::value_to_arraybuffer(rt, value);
      auto bytes = ForeignBytes{
          .len = static_cast<int32_t>(buffer.length(rt)),
          .data = buffer.data(rt),
      };

      // This buffer is constructed from foreign bytes. Rust scaffolding copies
      // the bytes, to make the RustBuffer.
      auto buf = rustbuffer_from_bytes(bytes);
      // Once it leaves this function, the buffer is immediately passed back
      // into Rust, where it's used to deserialize into the Rust versions of the
      // arguments. At that point, the copy is destroyed.
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
    auto arrayBuffer = jsi::ArrayBuffer(rt, payload);
    auto u8ctor = rt.global().getPropertyAsFunction(rt, "Uint8Array");
    auto view = u8ctor.callAsConstructor(rt, jsi::Value(rt, arrayBuffer))
                    .asObject(rt);
    if (buf.capacity != static_cast<uint64_t>(buf.len)) {
      view.setProperty(rt, "__ubrnRustCapacity",
                       jsi::Value(static_cast<double>(buf.capacity)));
    }
    return jsi::Value(rt, view);
  }
};

} // namespace {{ ci.cpp_namespace() }}
