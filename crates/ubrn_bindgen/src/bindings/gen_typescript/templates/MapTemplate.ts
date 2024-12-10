{{- self.import_infra("FfiConverterMap", "ffi-converters") }}
{%- let key_ffi_converter = key_type|ffi_converter_name(self) %}
{%- let value_ffi_converter = value_type|ffi_converter_name(self) %}
// FfiConverter for {{ type_name }}
const {{ ffi_converter_name }} = new FfiConverterMap({{ key_ffi_converter }}, {{ value_ffi_converter }});
