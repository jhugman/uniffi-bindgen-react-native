{%- if self.include_once_check("RustBufferTemplate.ts") %}
{{- self.add_import_from("RustBuffer", "ffi-types") }}
{{- self.add_import_from("FfiConverterArrayBuffer", "ffi-converters") }}
{%- endif -%}
