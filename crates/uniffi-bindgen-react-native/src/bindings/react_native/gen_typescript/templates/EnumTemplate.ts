{{- self.import_infra("AbstractFfiConverterArrayBuffer", "ffi-converters") -}}
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

const {{ ffi_converter_name }} = (() => {
    const ordinalConverter = FfiConverterInt32;
    type TypeName = {{ type_name }};
    class FFIConverter extends AbstractFfiConverterArrayBuffer<TypeName> {
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
            }
        }
        allocationSize(value: TypeName): number {
            return ordinalConverter.allocationSize(0);
        }
    }
    return new FFIConverter();
})();

{% else %}

// Enum: {{ type_name }}
{%- let kind_type_name = format!("{type_name}Kind") %}
export enum {{ kind_type_name }} {
    {%- for variant in e.variants() %}
    {{ variant|variant_name }} = "{{ variant.name() }}"
    {%- if !loop.last %},{% endif -%}
    {% endfor %}
}

{%- call ts::docstring(e, 0) %}
export type {{ type_name }} = {# space #}
{%- for variant in e.variants() %}
{%-   call ts::docstring(variant, 4) %}
    { kind: {{ kind_type_name }}.{{ variant|variant_name }}
    {%- if !variant.fields().is_empty() %}; value: { {# space #}
        {%- for field in variant.fields() %}
        {%- call ts::field_name(field, loop.index) %}: {{ field|type_name(ci) }}
        {%- if !loop.last %}; {% endif -%}
        {%- endfor %} }
    {%- endif %} }
    {%- if !loop.last %} |{% endif %}
{%- endfor %};

// FfiConverter for enum {{ type_name }}
const {{ ffi_converter_name }} = (() => {
    const ordinalConverter = FfiConverterInt32;
    type TypeName = {{ type_name }};
    class FFIConverter extends AbstractFfiConverterArrayBuffer<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (ordinalConverter.read(from)) {
                {%- for variant in e.variants() %}
                case {{ loop.index }}: return { kind: {{ kind_type_name }}.{{ variant|variant_name }}
                {%- if !variant.fields().is_empty() %}, value: {
                    {%- for field in variant.fields() %}
                    {% call ts::field_name(field, loop.index) %}: {{ field|read_fn }}(from)
                    {%- if !loop.last -%}, {% endif -%}{% endfor %} }
                {%- endif %} };
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        write(value: TypeName, into: RustBuffer): void {
            switch (value.kind) {
                {%- for variant in e.variants() %}
                case {{ kind_type_name }}.{{ variant|variant_name }}: {
                    ordinalConverter.write({{ loop.index }}, into);
                    {%- if !variant.fields().is_empty() %}
                    const inner = value.value;
                    {%-   for field in variant.fields() %}
                    {{ field|write_fn }}(inner.{% call ts::field_name(field, loop.index) %}, into);
                    {%-   endfor %}
                    {%- endif %}
                    return;
                }
                {%- endfor %}
                default:
                    // Throwing from here means that {{ kind_type_name }} hasn't matched an ordinal.
                    throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        allocationSize(value: TypeName): number {
            switch (value.kind) {
                {%- for variant in e.variants() %}
                case {{ kind_type_name }}.{{ variant|variant_name }}: {
                    {%- if !variant.fields().is_empty() %}
                    const inner = value.value;
                    let size = ordinalConverter.allocationSize({{ loop.index }});
                    {%- for field in variant.fields() %}
                    size += {{ field|allocation_size_fn }}(inner.{% call ts::field_name(field, loop.index) %});
                    {%- endfor %}
                    return size;
                    {%- else %}
                    return ordinalConverter.allocationSize({{ loop.index }});
                    {%- endif %}
                }
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
    }
    return new FFIConverter();
})();
{%- endif %}{# endif enum.is_flat() #}

{{- self.export_converter(ffi_converter_name) -}}
