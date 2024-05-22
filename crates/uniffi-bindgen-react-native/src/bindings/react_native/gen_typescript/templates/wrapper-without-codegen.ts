import { type UniffiRustFutureContinuationCallback } from 'uniffi-bindgen-react-native/async-rust-call';
import { type UniffiRustCallStatus } from 'uniffi-bindgen-react-native/rust-call';

interface NativeModuleInterface {
    {%- for func in ci.iter_ffi_functions_js_to_cpp() %}
    {%- let is_internal = func.is_internal() %}
    {{ func.name() }}(
      {%- call ts::arg_list_ffi_decl(func) %}):
      {%- match func.return_type() %}{% when Some with (return_type) %} {{ return_type.borrow()|ffi_type_name_for_cpp(is_internal) }}{% when None %} void{% endmatch %};
  {%- endfor %}
}

// Casting globalThis to any allows us to look for `{{ config.cpp_module }}`
// if it was added via JSI.
//
// The empty object is there for testing purposes only, and may be removed.
export default ((globalThis as any).{{ config.cpp_module}} ?? {}) as NativeModuleInterface;

{%- import "macros.ts" as ts %}
