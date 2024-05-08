
struct RustBuffer {
    int32_t capacity;
    int32_t len;
    uint8_t *data;
};

namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<RustBuffer> {
  static RustBuffer fromJs(jsi::Runtime &rt, const jsi::Value &value) {
    try {
      throw jsi::JSError(rt, "Unimplemented RustBuffer fromJs");
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, RustBuffer value) {
    throw jsi::JSError(rt, "Unimplemented RustBuffer toJs");
  }
};

} // namespace uniffi_jsi
