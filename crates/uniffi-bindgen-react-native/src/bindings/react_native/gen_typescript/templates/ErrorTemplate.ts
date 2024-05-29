{{- self.import_infra("rustCallWithError", "rust-call") }}

{%- call ts::docstring(e, 0) %}
export class {{ type_name }} extends Error {
    constructor(message: string) {
        super(message);
    }
    {%- if e.is_flat() %}
    {%-   for variant in e.variants() %}
    {%-    let var_name = variant.name()|class_name(ci) %}
    static {{ var_name }}: typeof _{{ type_name }}_{{ var_name }};
    {% endfor -%}
    {% else %}
    // non-flat errors aren't implemented yet.
    {%- endif %}
}
{%- if e.is_flat() %}
{%-   for variant in e.variants() %}
{%-    call ts::docstring(variant, 4) %}
{%-    let var_name = variant.name()|class_name(ci) %}
class _{{ type_name }}_{{ var_name }} extends {{ type_name }} {
    constructor(message: string) { super(message); }
}
{{ type_name }}.{{ var_name }} = _{{ type_name }}_{{ var_name }};
{% endfor -%}
{%- endif %}


const {{ ffi_converter_name }} = (() => {
    const intConverter = FfiConverterInt32;
    const stringConverter = FfiConverterString;

    type TypeName = {{ type_name }};
    class FfiConverter extends AbstractFfiConverterArrayBuffer<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (intConverter.read(from)) {
            {%- if e.is_flat() %}
            {%-   for variant in e.variants() %}
                case {{ loop.index }}: return new {{ type_name }}.{{ variant.name()|class_name(ci) }}(
                    stringConverter.read(from)
                );
            {%    endfor %}
            {%- else %}
                // non-flat errors aren't implement yet.
            {%  endif %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        write(value: TypeName, into: RustBuffer): void {
            {%- if e.is_flat() %}

            {%- for variant in e.variants() %}
            if (value instanceof {{ type_name }}.{{ variant.name()|class_name(ci) }}) {
                intConverter.write({{ loop.index0 }}, into);
            } else
            {%- endfor %} {
                throw new UniffiInternalError.UnexpectedEnumCase();
            }
            {%- else %}
            throw new Error("Method not implemented.")
            {%- endif %}
        }
        allocationSize(value: TypeName): number {
            throw new Error("Method not implemented.")
        }
    }
    return new FfiConverter();
})();
