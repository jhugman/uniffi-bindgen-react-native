{%- let struct_name = ffi_struct.name()|ffi_struct_name %}
namespace uniffi_jsi {
using namespace facebook;

template <> struct Bridging<{{ struct_name }}> {
  static {{ struct_name }} fromJs(jsi::Runtime &rt, const jsi::Value &value) {
    try {
      throw jsi::JSError(rt, "VTable struct unimplemented {{ struct_name }}");
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }
};

} // namespace uniffi_jsi
