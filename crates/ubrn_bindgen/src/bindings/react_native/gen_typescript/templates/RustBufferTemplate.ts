{%- if self.include_once_check("RustBufferTemplate.ts") %}
{{- self.import_infra("RustBuffer", "ffi-types") }}
{{- self.import_infra("AbstractFfiConverterArrayBuffer", "ffi-converters") }}
{%- endif -%}
