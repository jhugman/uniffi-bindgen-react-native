{{- self.import_infra("UniffiError", "errors") }}
{{- self.import_infra("uniffiTypeNameSymbol", "symbols") }}
{{- self.import_infra("variantOrdinalSymbol", "symbols") }}
{%- let flat = e.is_flat() %}
{%- if flat %}

// Flat error type: {{ decl_type_name }}
{%- let type_name__Tags = format!("{type_name}_Tags") %}
export enum {{ type_name__Tags }} {
    {%- for variant in e.variants() %}
    {{ variant|variant_name }} = "{{ variant.name() }}"
    {%- if !loop.last %},{% endif -%}
    {% endfor %}
}

{%- call ts::docstring(e, 0) %}
export const {{ decl_type_name }} = (() => {
    {%- for variant in e.variants() %}
    {%-   call ts::docstring(variant, 4) %}
    {%-   let variant_name = variant.name()|class_name(ci) %}
    class {{ variant_name }} extends UniffiError {
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

        public readonly tag = {{ type_name__Tags }}.{{ variant|variant_name }};

        constructor(message: string) {
            super("{{ type_name }}", "{{ variant_name }}", message);
        }

        static instanceOf(e: any): e is {{ variant_name }} {
            return (
                instanceOf(e) && (e as any)[variantOrdinalSymbol] === {{ loop.index }}
            );
        }
    }
    {%- endfor %}

    // Utility function which does not rely on instanceof.
    function instanceOf(e: any): e is {{ type_name }} {
        return (e as any)[uniffiTypeNameSymbol] === "{{ decl_type_name }}";
    }
    return {
        {%- for variant in e.variants() %}
        {{   variant.name()|class_name(ci) }},
        {%- endfor %}
        instanceOf,
    };
})();

// Union type for {{ type_name }} error type.
{% call ts::docstring(e, 0) %}
{% call ts::type_omit_instanceof(type_name, decl_type_name) %}

const {{ ffi_converter_name }} = (() => {
    const intConverter = FfiConverterInt32;
    type TypeName = {{ type_name }};
    {{- self.import_infra("AbstractFfiConverterArrayBuffer", "ffi-converters") }}
    class FfiConverter extends AbstractFfiConverterArrayBuffer<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (intConverter.read(from)) {
            {%-   for variant in e.variants() %}
                case {{ loop.index }}: return new {{ decl_type_name }}.{{ variant.name()|class_name(ci) }}(
                    {%- if flat %}FfiConverterString.read(from)
                    {%- else %}
                    {%-   for field in variant.fields() %}
                    {{      field|ffi_converter_name(self) }}.read(from)
                    {%-     if !loop.last %}, {% endif %}
                    {%-   endfor %}
                    {%- endif %}
                );
            {%    endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        write(value: TypeName, into: RustBuffer): void {
            const obj = value as any;
            const index = obj[variantOrdinalSymbol] as number;
            intConverter.write(index, into);
            {%- if !flat %}
            switch (index) {
                {%-   for variant in e.variants() %}
                case {{ loop.index }}:
                {%-     for field in variant.fields() %}
                    {{ field|ffi_converter_name(self) }}.write(obj.{{ field.name()|var_name }} as {{ field|type_name(self) }}, into);
                {%-     endfor -%}
                    break;
                {%-   endfor %}
                    default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
            {%- endif %}
        }
        allocationSize(value: TypeName): number {
            {%- if flat %}
            return intConverter.allocationSize(0);
            {%- else %}
            const obj = value as any;
            const index = obj[variantOrdinalSymbol] as number;
            switch (index) {
                {%-   for variant in e.variants() %}
                case {{ loop.index }}:
                    return (intConverter.allocationSize({{ loop.index }})
                {%-     for field in variant.fields() %} + {# space #}
                    {{ field|ffi_converter_name(self) }}.allocationSize(obj.{{ field.name()|var_name }} as {{ field|type_name(self) }})
                {%-     endfor -%}
                    );
                {%-   endfor %}
                    default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
            {%- endif %}
        }
    }
    return new FfiConverter();
})();
{%- else %}
// Error type: {{ decl_type_name }}
{% let superclass = "UniffiError" %}
{% let is_error = true %}
{%- include "TaggedEnumTemplate.ts" %}
{%- endif %}
