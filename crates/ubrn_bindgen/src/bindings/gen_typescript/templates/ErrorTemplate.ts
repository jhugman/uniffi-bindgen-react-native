{%- macro flat_error(e) %}
{%- let type_name = e.ts_name %}
{%- let type_name__Tags = format!("{type_name}_Tags") %}

// Flat error type: {{ type_name }}
export enum {{ type_name__Tags }} {
    {%- for variant in e.variants %}
    {{ variant.name }} = "{{ variant.name }}"
    {%- if !loop.last %},{% endif -%}
    {% endfor %}
}

{%- if let Some(ds) = e.docstring %}
{{ ds }}
{%- endif %}
export const {{ type_name }} = (() => {
    {%- for variant in e.variants %}
    {%- if let Some(ds) = variant.docstring %}
{{ ds }}
    {%- endif %}
    class {{ variant.name }} extends UniffiError {
        /**
         * @private
         * This field is private and should not be used.
         */
        readonly [uniffiTypeNameSymbol]: string = "{{ type_name }}";
        /**
         * @private
         * This field is private and should not be used.
         */
        readonly [variantOrdinalSymbol] = {{ loop.index }};

        readonly tag = {{ type_name__Tags }}.{{ variant.name }};

        constructor(message: string) {
            super("{{ type_name }}", "{{ variant.name }}", message);
        }

        static instanceOf(e: any): e is {{ variant.name }} {
            return (
                instanceOf(e) && (e as any)[variantOrdinalSymbol] === {{ loop.index }}
            );
        }
    }
    {%- endfor %}

    // Utility function which does not rely on instanceof.
    function instanceOf(e: any): e is {{ type_name }} {
        return (e as any)[uniffiTypeNameSymbol] === "{{ type_name }}";
    }
    return {
        {%- for variant in e.variants %}
        {{   variant.name }},
        {%- endfor %}
        instanceOf,
    };
})();

// Union type for {{ type_name }} error type.
{%- if let Some(ds) = e.docstring %}
{{ ds }}
{%- endif %}
export type {{ type_name }} = InstanceType<
    typeof {{ type_name }}[{%- for variant in e.variants %}'{{ variant.name }}'{% if !loop.last %} | {% endif %}{%- endfor %}]
>;

const {{ e.ffi_converter_name }} = (() => {
    const intConverter = FfiConverterInt32;
    type TypeName = {{ type_name }};
    class FfiConverter extends AbstractFfiConverterByteArray<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (intConverter.read(from)) {
            {%-   for variant in e.variants %}
                case {{ loop.index }}: return new {{ type_name }}.{{ variant.name }}(
                    FfiConverterString.read(from)
                );
            {%    endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        write(value: TypeName, into: RustBuffer): void {
            const obj = value as any;
            const index = obj[variantOrdinalSymbol] as number;
            intConverter.write(index, into);
        }
        allocationSize(value: TypeName): number {
            return intConverter.allocationSize(0);
        }
    }
    return new FfiConverter();
})();
{%- endmacro %}
