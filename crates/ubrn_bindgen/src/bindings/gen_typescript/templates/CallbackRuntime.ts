{%-   if ci.has_async_callbacks() %}
// These types are part of the async callback machinery.
// They are discarded at compile time so should not affect the bundle size.
{# space #}
{%-     for ffi_struct in ci.iter_ffi_structs_for_callbacks() %}
type {{ ffi_struct.name()|ffi_struct_name }} = {
{%-       for field in ffi_struct.fields() %}
  {{ field.name()|var_name }}: {{ field.type_().borrow()|ffi_type_name }};
{%-       endfor %}
};
{%-     endfor %}

{%-     for callback in ci.iter_ffi_callback_literals() %}
type {{ callback.name()|ffi_callback_name }} = (
{%-       for arg in callback.arguments_no_return() %}
{{- arg.name()|var_name }}: {{ arg.type_().borrow()|ffi_type_name }}{% if !loop.last %}, {% endif %}
{%-       endfor %}) => void;
{%-     endfor %}
{%- endif %}
