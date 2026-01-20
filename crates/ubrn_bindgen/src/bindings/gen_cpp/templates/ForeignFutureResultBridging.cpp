// Generate Bridging templates for internal UniffiForeignFutureResult structs
// These are used by async callbacks but not exposed in ffi_definitions()
namespace {{ ci.cpp_namespace() }} {
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

{%- for type_suffix in ["U8", "I8", "U16", "I16", "U32", "I32", "U64", "I64", "F32", "F64"] %}
{%- let struct_name = "UniffiForeignFutureResult" ~ type_suffix %}

#ifndef BRIDGING_{{ struct_name }}_DEFINED
#define BRIDGING_{{ struct_name }}_DEFINED
template <> struct Bridging<{{ struct_name }}> {
  static {{ struct_name }} fromJs(jsi::Runtime &rt,
    std::shared_ptr<CallInvoker> callInvoker,
    const jsi::Value &jsValue
  ) {
    if (!jsValue.isObject()) {
      throw jsi::JSError(rt, "Expected an object for {{ struct_name }}");
    }
    auto jsObject = jsValue.getObject(rt);
    {{ struct_name }} rsObject;
    
    // Convert return_value field
    rsObject.return_value = uniffi_jsi::Bridging<decltype(rsObject.return_value)>::fromJs(
        rt, callInvoker, jsObject.getProperty(rt, "return_value")
    );
    
    // Convert call_status field 
    rsObject.call_status = Bridging<RustCallStatus>::fromJs(
        rt, callInvoker, jsObject.getProperty(rt, "call_status")
    );
    
    return rsObject;
  }

  static jsi::Value toJs(jsi::Runtime &rt,
    std::shared_ptr<CallInvoker> callInvoker,
    const {{ struct_name }} &rsValue
  ) {
    auto jsObject = jsi::Object(rt);
    
    jsObject.setProperty(rt, "return_value",
      uniffi_jsi::Bridging<decltype(rsValue.return_value)>::toJs(rt, callInvoker, rsValue.return_value)
    );
    
    jsObject.setProperty(rt, "call_status",
      Bridging<RustCallStatus>::toJs(rt, callInvoker, rsValue.call_status)
    );
    
    return jsObject;
  }
};
#endif // BRIDGING_{{ struct_name }}_DEFINED

{%- endfor %}

// Special case for Void - no return_value field
#ifndef BRIDGING_UniffiForeignFutureResultVoid_DEFINED
#define BRIDGING_UniffiForeignFutureResultVoid_DEFINED
template <> struct Bridging<UniffiForeignFutureResultVoid> {
  static UniffiForeignFutureResultVoid fromJs(jsi::Runtime &rt,
    std::shared_ptr<CallInvoker> callInvoker,
    const jsi::Value &jsValue
  ) {
    if (!jsValue.isObject()) {
      throw jsi::JSError(rt, "Expected an object for UniffiForeignFutureResultVoid");
    }
    auto jsObject = jsValue.getObject(rt);
    UniffiForeignFutureResultVoid rsObject;
    
    // Convert call_status field 
    rsObject.call_status = Bridging<RustCallStatus>::fromJs(
        rt, callInvoker, jsObject.getProperty(rt, "call_status")
    );
    
    return rsObject;
  }

  static jsi::Value toJs(jsi::Runtime &rt,
    std::shared_ptr<CallInvoker> callInvoker,
    const UniffiForeignFutureResultVoid &rsValue
  ) {
    auto jsObject = jsi::Object(rt);
    
    jsObject.setProperty(rt, "call_status",
      Bridging<RustCallStatus>::toJs(rt, callInvoker, rsValue.call_status)
    );
    
    return jsObject;
  }
};
#endif // BRIDGING_UniffiForeignFutureResultVoid_DEFINED

// Special case for RustBuffer - uses module namespace Bridging, not uniffi_jsi
#ifndef BRIDGING_UniffiForeignFutureResultRustBuffer_DEFINED
#define BRIDGING_UniffiForeignFutureResultRustBuffer_DEFINED
template <> struct Bridging<UniffiForeignFutureResultRustBuffer> {
  static UniffiForeignFutureResultRustBuffer fromJs(jsi::Runtime &rt,
    std::shared_ptr<CallInvoker> callInvoker,
    const jsi::Value &jsValue
  ) {
    if (!jsValue.isObject()) {
      throw jsi::JSError(rt, "Expected an object for UniffiForeignFutureResultRustBuffer");
    }
    auto jsObject = jsValue.getObject(rt);
    UniffiForeignFutureResultRustBuffer rsObject;
    
    // Convert return_value field - RustBuffer uses module namespace Bridging
    rsObject.return_value = Bridging<RustBuffer>::fromJs(
        rt, callInvoker, jsObject.getProperty(rt, "return_value")
    );
    
    // Convert call_status field 
    rsObject.call_status = Bridging<RustCallStatus>::fromJs(
        rt, callInvoker, jsObject.getProperty(rt, "call_status")
    );
    
    return rsObject;
  }

  static jsi::Value toJs(jsi::Runtime &rt,
    std::shared_ptr<CallInvoker> callInvoker,
    const UniffiForeignFutureResultRustBuffer &rsValue
  ) {
    auto jsObject = jsi::Object(rt);
    
    jsObject.setProperty(rt, "return_value",
      Bridging<RustBuffer>::toJs(rt, callInvoker, rsValue.return_value)
    );
    
    jsObject.setProperty(rt, "call_status",
      Bridging<RustCallStatus>::toJs(rt, callInvoker, rsValue.call_status)
    );
    
    return jsObject;
  }
};
#endif // BRIDGING_UniffiForeignFutureResultRustBuffer_DEFINED

} // namespace {{ ci.cpp_namespace() }}

