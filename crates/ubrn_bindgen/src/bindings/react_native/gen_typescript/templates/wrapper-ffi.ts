import { type UniffiRustFutureContinuationCallback as RuntimeUniffiRustFutureContinuationCallback } from 'uniffi-bindgen-react-native/async-rust-call';
import { type StructuralEquality as UniffiStructuralEquality } from 'uniffi-bindgen-react-native/type-utils';
import { type UniffiRustCallStatus } from 'uniffi-bindgen-react-native/rust-call';
import { UniffiReferenceHolder } from 'uniffi-bindgen-react-native/callbacks';

interface NativeModuleInterface {
    {%- for func in ci.iter_ffi_functions_js_to_cpp() %}
    {%- let is_internal = func.is_internal() %}
    {{ func.name() }}(
      {%- call ts::arg_list_ffi_decl(func) %}):
      {%- match func.return_type() %}{% when Some with (return_type) %} {{ return_type.borrow()|ffi_type_name_for_cpp(is_internal) }}{% when None %} void{% endmatch %};
  {%- endfor %}
}

// Casting globalThis to any allows us to look for `{{ config.cpp_module() }}`
// if it was added via JSI.
//
// We use a getter here rather than simply `globalThis.{{ config.cpp_module() }}` so that
// if/when the startup sequence isn't just so, an empty value isn't inadvertantly cached.
const getter: () => NativeModuleInterface = () => (globalThis as any).{{ config.cpp_module() }};
export default getter;

// Structs and function types for calling back into Typescript from Rust.
{%- for def in ci.ffi_definitions() %}
{%- match def %}
{%- when FfiDefinition::CallbackFunction(callback) %}
type {{ callback.name()|ffi_callback_name }} = (
{%-   for arg in callback.arguments() %}
{{- arg.name()|var_name }}: {{ arg.type_().borrow()|ffi_type_name }}{% if !loop.last %}, {% endif %}
{%-   endfor %}
{%-   if callback.has_rust_call_status_arg() -%}
{%      if callback.arguments().len() > 0 %}, {% endif %}callStatus: UniffiRustCallStatus
{%-   endif %}) => {# space #}
{%-   match callback.return_type() %}
{%-     when Some(return_type) %}{{ return_type|ffi_type_name }}
{%-     when None %}void
{%-   endmatch %};
{%- when FfiDefinition::Struct(ffi_struct) %}
export type {{ ffi_struct.name()|ffi_struct_name }} = {
  {%- for field in ffi_struct.fields() %}
  {{ field.name()|var_name }}: {{ field.type_().borrow()|ffi_type_name }};
  {%- endfor %}
};
{%- else %}
{%- endmatch %}
{%- endfor %}

// UniffiRustFutureContinuationCallback is generated as part of the component interface's
// ffi_definitions. However, we need it in the runtime.
// We could:
// (a) do some complicated template logic to ensure the declaration is not generated here (possible)
// (b) import the generated declaration into the runtime (m a y b e) or…
// (c) generate the declaration anyway, and use a different declaration in the runtime.
//
// We chose (c) here as the simplest. In addition, we perform a compile time check that
// the two versions of `UniffiRustFutureContinuationCallback` are structurally equivalent.
//
// If you see the error:
// ```
// Type 'true' is not assignable to type 'false'.(2322)
// ```
// Then a new version of uniffi has changed the signature of the callback. Most likely, code in
// `typescript/src/async-rust-call.ts` will need to be changed.
//
// If you see the error:
// ```
// Cannot find name 'UniffiRustFutureContinuationCallback'. Did you mean 'RuntimeUniffiRustFutureContinuationCallback'?(2552)
// ```
// then you may not be using callbacks or promises, and uniffi is now not generating Futures and callbacks.
// You should not generate this if that is the case.
//
// ('You' being the bindings generator maintainer).
const isRustFutureContinuationCallbackTypeCompatible: UniffiStructuralEquality<
  RuntimeUniffiRustFutureContinuationCallback,
  UniffiRustFutureContinuationCallback
> = true;

{%- import "macros.ts" as ts %}
