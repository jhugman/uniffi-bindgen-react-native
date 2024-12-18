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
      auto rbuf = Bridging<RustBuffer>::toJs(rt, callInvoker,
                                                         status.error_buf);
      statusObject.setProperty(rt, "errorBuf", rbuf);
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
