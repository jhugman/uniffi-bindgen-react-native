
struct Handle {};

namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<Handle> {
  static Handle fromJs(jsi::Runtime &rt, const jsi::Value &value) {
    try {
      throw jsi::JSError(rt, "Unimplemented");
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, Handle value) {
    throw jsi::JSError(rt, "Unimplemented");
  }
};

} // namespace uniffi_jsi
