{%- macro simple_wrapper(wrapper) %}
// FfiConverter for {{ wrapper.type_label }}
const {{ wrapper.ffi_converter_name }} = new {{ wrapper.infra_class }}(
  {%- for conv in wrapper.inner_converters %}{{ conv }}{% if !loop.last %}, {% endif %}{%- endfor %});
{%- endmacro %}
