{%- include "StringHelper.ts" %}
{%- include "Int32Helper.ts" %}
{{- self.import_infra("rustCallWithError", "rust-call") }}

{%- call ts::docstring(e, 0) %}
export class {{ type_name }} extends Error {
    private constructor(message: string) {
        super(message);
    }
    {%- if e.is_flat() %}
    {%-   for variant in e.variants() %}
    {%-    call ts::docstring(variant, 4) %}
    {%-    let var_name = variant.name()|class_name(ci) %}
    static {{ var_name }} = class {{ var_name }} extends {{ type_name }} {
        constructor(message: string) { super(message); }
    }
    {% endfor -%}
    {% else %}
    // non-flat errors aren't implement yet.
    {%- endif %}
}

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
            throw new Error("Method not implemented.")
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
