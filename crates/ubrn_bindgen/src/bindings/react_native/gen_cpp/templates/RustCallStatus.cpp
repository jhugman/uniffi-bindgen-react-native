constexpr int8_t UNIFFI_CALL_STATUS_OK = 0;
constexpr int8_t UNIFFI_CALL_STATUS_ERROR = 1;
constexpr int8_t UNIFFI_CALL_STATUS_PANIC = 2;

struct RustCallStatus {
    int8_t code;
    RustBuffer error_buf;
};

namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<RustCallStatus> {
  static jsi::Value jsSuccess(jsi::Runtime &rt) {
    auto statusObject = jsi::Object(rt);
    statusObject.setProperty(rt, "code", jsi::Value(rt, UNIFFI_CALL_STATUS_OK));
    return statusObject;
  }
  static RustCallStatus rustSuccess(jsi::Runtime &rt) {
    return { UNIFFI_CALL_STATUS_OK };
  }
  static void copyIntoJs(jsi::Runtime &rt, const RustCallStatus status, const jsi::Value &jsStatus) {
    auto statusObject = jsStatus.asObject(rt);
    if (status.error_buf.data != nullptr) {
        auto rbuf = uniffi_jsi::Bridging<RustBuffer>::toJs(rt, status.error_buf);
        statusObject.setProperty(rt, "errorBuf", rbuf);
    }
    if (status.code != UNIFFI_CALL_STATUS_OK) {
        auto code = uniffi_jsi::Bridging<uint8_t>::toJs(rt, status.code);
        statusObject.setProperty(rt, "code", code);
    }
  }
  static void copyFromJs(jsi::Runtime &rt, const jsi::Value &jsStatus, RustCallStatus *status) {
    auto statusObject = jsStatus.asObject(rt);
    if (statusObject.hasProperty(rt, "errorBuf")) {
        auto rbuf = statusObject.getProperty(rt, "errorBuf");
        status->error_buf = uniffi_jsi::Bridging<RustBuffer>::fromJs(rt, rbuf);
    }
    if (statusObject.hasProperty(rt, "code")) {
        auto code = statusObject.getProperty(rt, "code");
        status->code = uniffi_jsi::Bridging<uint8_t>::fromJs(rt, code);
    }
  }
};

} // namespace uniffi_jsi
