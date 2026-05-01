{%- let ns = ci.cpp_namespace() %}
namespace {{ ns }} {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

template <> struct Bridging<RustCallStatus> {
  static jsi::Value jsSuccess(jsi::Runtime &rt) {
    auto statusObject = jsi::Object(rt);
    statusObject.setProperty(rt, "code", jsi::Value(rt, UNIFFI_CALL_STATUS_OK));
    return statusObject;
  }
  static RustCallStatus rustSuccess(jsi::Runtime &rt) {
    return {UNIFFI_CALL_STATUS_OK};
  }
  static void copyIntoJs(jsi::Runtime &rt,
                         std::shared_ptr<CallInvoker> callInvoker,
                         const RustCallStatus status,
                         const jsi::Value &jsStatus) {
    auto statusObject = jsStatus.asObject(rt);
    if (status.error_buf.data != nullptr) {
      // The error path is NOT wrapped in the codegen-emitted try/finally that
      // covers normal returns: `errorBuf` is read by the runtime's call-status
      // dispatcher (rust-call.ts) which throws straight to the user without
      // ever calling `rustbuffer_free`. Switching this site to view-handoff
      // would leak the Rust allocation, so we keep the copy semantics here:
      // copy the bytes into a JS-owned ArrayBuffer and free the Rust buffer
      // immediately. The errorBuf is small (a serialized error variant) and
      // only allocated on the cold error path, so the boundary copy is cheap.
      auto len = static_cast<size_t>(status.error_buf.len);
      uint8_t *bytes = new uint8_t[len];
      std::memcpy(bytes, status.error_buf.data, len);
      auto payload = std::make_shared<uniffi_jsi::CMutableBuffer>(bytes, len);
      auto arrayBuffer = jsi::ArrayBuffer(rt, payload);
      auto u8ctor = rt.global().getPropertyAsFunction(rt, "Uint8Array");
      auto view = u8ctor.callAsConstructor(rt, jsi::Value(rt, arrayBuffer));
      statusObject.setProperty(rt, "errorBuf", view);
      Bridging<RustBuffer>::rustbuffer_free(status.error_buf);
    }
    if (status.code != UNIFFI_CALL_STATUS_OK) {
      auto code =
          uniffi_jsi::Bridging<uint8_t>::toJs(rt, callInvoker, status.code);
      statusObject.setProperty(rt, "code", code);
    }
  }

  static RustCallStatus fromJs(jsi::Runtime &rt,
                               std::shared_ptr<CallInvoker> invoker,
                               const jsi::Value &jsStatus) {
    RustCallStatus status;
    auto statusObject = jsStatus.asObject(rt);
    if (statusObject.hasProperty(rt, "errorBuf")) {
      auto rbuf = statusObject.getProperty(rt, "errorBuf");
      status.error_buf =
          Bridging<RustBuffer>::fromJs(rt, invoker, rbuf);
    }
    if (statusObject.hasProperty(rt, "code")) {
      auto code = statusObject.getProperty(rt, "code");
      status.code = uniffi_jsi::Bridging<uint8_t>::fromJs(rt, invoker, code);
    }
    return status;
  }

  static void copyFromJs(jsi::Runtime &rt, std::shared_ptr<CallInvoker> invoker,
                         const jsi::Value &jsStatus, RustCallStatus *status) {
    auto statusObject = jsStatus.asObject(rt);
    if (statusObject.hasProperty(rt, "errorBuf")) {
      auto rbuf = statusObject.getProperty(rt, "errorBuf");
      status->error_buf =
          Bridging<RustBuffer>::fromJs(rt, invoker, rbuf);
    }
    if (statusObject.hasProperty(rt, "code")) {
      auto code = statusObject.getProperty(rt, "code");
      status->code = uniffi_jsi::Bridging<uint8_t>::fromJs(rt, invoker, code);
    }
  }
};

} // namespace {{ ns }}
