{{- self.import_infra("AbstractFfiConverterByteArray", "ffi-converters") -}}
{{- self.import_infra("FfiConverterInt32", "ffi-converters") -}}
{{- self.import_infra("UniffiInternalError", "errors") -}}

{% if e.is_flat() %}
{%- call ts::docstring(e, 0) %}
export enum {{ type_name }} {
    {%- for variant in e.variants() %}
    {%- call ts::docstring(variant, 4) %}
    {{ variant|variant_name }}
    {%- match e.variant_discr_type() %}
    {%- when Some with (_) %} = {{ e|variant_discr_literal(loop.index0, ci) }}
    {%- else %}{% endmatch %}
    {%- if !loop.last %},{% endif -%}
    {% endfor %}
}
{%- let tm = e.uniffi_trait_methods() %}
{%- if e.has_uniffi_traits() %}

export namespace {{ type_name }} {
{% call ts::uniffi_trait_methods_value_receiver(tm, ffi_converter_name, type_name, "    export function ", "") %}
}
{%- endif %}

const {{ ffi_converter_name }} = (() => {
    const ordinalConverter = FfiConverterInt32;
    type TypeName = {{ type_name }};
    class FFIConverter extends AbstractFfiConverterByteArray<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (ordinalConverter.read(from)) {
                {%- for variant in e.variants() %}
                case {{ loop.index0 + 1}}: return {{ type_name }}.{{ variant|variant_name }};
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        write(value: TypeName, into: RustBuffer): void {
            switch (value) {
                {%- for variant in e.variants() %}
                case {{ type_name }}.{{ variant|variant_name }}: return ordinalConverter.write({{ loop.index0 + 1 }}, into);
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        allocationSize(value: TypeName): number {
            switch (value) {
                {%- for variant in e.variants() %}
                case {{ type_name }}.{{ variant|variant_name }}: return ordinalConverter.allocationSize(0);
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
    }
    return new FFIConverter();
})();

{% else %}
{{- self.import_infra("UniffiEnum", "enums") }}
{%- let superclass = "UniffiEnum" %}
{% let is_error = false %}
{%- include "TaggedEnumTemplate.ts" %}
{%- endif %}{# endif e.is_flat() #}

{{- self.export_converter(ffi_converter_name) -}}
