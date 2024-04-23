{{- self.add_import_from("FfiConverterArray", "ffi-converters") }}
{%- let item_ffi_converter = inner_type|ffi_converter_name %}
// FfiConverter for {{ type_name }}
const {{ ffi_converter_name }} = new FfiConverterArray({{ item_ffi_converter }});
