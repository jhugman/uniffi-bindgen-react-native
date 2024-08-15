{{- self.import_infra("FfiConverterOptional", "ffi-converters") }}
{%- let item_ffi_converter = inner_type|ffi_converter_name(self) %}
// FfiConverter for {{ type_name }}
const {{ ffi_converter_name }} = new FfiConverterOptional({{ item_ffi_converter }});
