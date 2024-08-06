// This file was autogenerated by some hot garbage in the `uniffi` crate.
// Trust me, you don't want to mess with it!
import nativeModule, {
  {%- for def in ci.iter_ffi_definitions_exported_by_ts() %}
  {%- match def %}
  {%- when FfiDefinition::CallbackFunction(ffi_func) %}
  type {{ ffi_func.name()|ffi_callback_name }},
  {%- when FfiDefinition::Struct(ffi_struct) %}
  type {{ ffi_struct.name()|ffi_struct_name }},
  {%- else %}
  {%- endmatch %}
  {%- endfor %}
} from "./{{ module.ts_ffi() }}";

{%- for entry in self.type_imports.borrow() %}
{%-   let file = entry.0 %}
{%-   let things = entry.1 %}
import {
{%-   for thing in things %}
{%-     match thing %}
{%-       when Imported::TSType with (type_) %}
  type {{ type_ }}
{%-       when Imported::JSType with (type_) %}
  {{ type_ }}
{%-     endmatch %}
{%-     if !loop.last %}, {% endif %}
{%-   endfor %} } from "{{ file }}";
{%- endfor %}

// Get converters from the other files, if any.
{%- for entry in self.imported_converters.borrow() %}
import {{ entry.0.1 }} from "{{ entry.0.0 }}";
{%- endfor %}
{%- for entry in self.imported_converters.borrow() %}
{%-   let converters = entry.1 %}
const {
{%-   for converter in converters %}
        {{- converter }},
{%-   endfor %}
} = {{ entry.0.1 }}.converters;
{%- endfor %}

{%- call ts::docstring_value(ci.namespace_docstring(), 0) %}

{%- import "macros.ts" as ts %}
// Public interface members begin here.
{{ type_helper_code }}

{% include "InitializationTemplate.ts" %}

export default Object.freeze({
  initialize: uniffiEnsureInitialized,
  {%- if !self.exported_converters.is_empty() %}
  converters: {
  {%- for converter in self.exported_converters.borrow() %}
    {{ converter }},
  {%- endfor %}
  }{% endif %}
});
