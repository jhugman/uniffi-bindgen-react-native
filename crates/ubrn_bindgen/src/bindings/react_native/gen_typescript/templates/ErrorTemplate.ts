{{- self.import_infra("UniffiError", "errors") }}
{%- let instance_of = "instanceOf" %}
{%- let flat = e.is_flat() %}
{%- if flat %}
// Flat error type: {{ decl_type_name }}
{%- else %}
// Error type: {{ decl_type_name }}
{%- endif %}
{%- call ts::docstring(e, 0) %}
export const {{ decl_type_name }} = (() => {
    {%- for variant in e.variants() %}
    {%-   call ts::docstring(variant, 4) %}
    {%-   let variant_name = variant.name()|class_name(ci) %}
    class {{ variant_name }} extends UniffiError {
        constructor(
            {%- if flat %}message: string
            {%- else %}
            {%-   for field in variant.fields() %}
            public readonly {{ field.name()|var_name }}: {{ field|type_name(ci) }}
            {%-     match field.default_value() %}
            {%-       when Some with(literal) %} = {{ literal|render_literal(field, ci) }}
            {%-     else %}
            {%-     endmatch -%}
            {%-     if !loop.last %}, {% endif %}
            {%-   endfor %}
            {%- endif -%}
        ) {
            super("{{ decl_type_name }}", "{{ variant_name }}", {{ loop.index }}
                {%- if flat %}, message
                {%- endif %});
        }

        {%- if !flat %}
        toString(): string {
            return ["{{ decl_type_name }}.{{ variant_name }}:"
            {%-   for field in variant.fields() %}, {# space #}
            `{{ field.name()|var_name }}=${this.{{ field.name()|var_name }}}`
            {%-   endfor %}].join(" ");
        }
        {%- endif -%}

        static {{ instance_of }}(e: any): e is {{ variant_name }} {
            return (
                {{ instance_of }}(e) && (e as any).__variant === {{ loop.index }}
            );
        }
    }
    {%- endfor %}

    // Utility function which does not rely on instanceof.
    function {{ instance_of }}(e: any): e is {# space #}
    {%- for variant in e.variants() %}
    {{-   variant.name()|class_name(ci) }}
    {%-   if !loop.last %} | {% endif -%}
    {%- endfor %} {
        return (e as any).__uniffiTypeName === "{{ decl_type_name }}";
    }
    return {
        {%- for variant in e.variants() %}
        {{   variant.name()|class_name(ci) }},
        {%- endfor %}
        {{ instance_of }},
    };
})();

// Union type for {{ type_name }} error type.
{% call ts::docstring(e, 0) %}
{% call ts::type_omit_instanceof(type_name, decl_type_name) %}

const {{ ffi_converter_name }} = (() => {
    const intConverter = FfiConverterInt32;

    type TypeName = {{ type_name }};
    class FfiConverter extends AbstractFfiConverterArrayBuffer<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (intConverter.read(from)) {
            {%-   for variant in e.variants() %}
                case {{ loop.index }}: return new {{ decl_type_name }}.{{ variant.name()|class_name(ci) }}(
                    {%- if flat %}FfiConverterString.read(from)
                    {%- else %}
                    {%-   for field in variant.fields() %}
                    {{      field|ffi_converter_name }}.read(from)
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
            const index = obj.__variant as number;
            intConverter.write(index, into);
            {%- if !flat %}
            switch (index) {
                {%-   for variant in e.variants() %}
                case {{ loop.index }}:
                {%-     for field in variant.fields() %}
                    {{ field|ffi_converter_name }}.write(obj.{{ field.name()|var_name }} as {{ field|type_name(ci) }}, into);
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
            const index = obj.__variant as number;
            switch (index) {
                {%-   for variant in e.variants() %}
                case {{ loop.index }}:
                    return (intConverter.allocationSize({{ loop.index }})
                {%-     for field in variant.fields() %} + {# space #}
                    {{ field|ffi_converter_name }}.allocationSize(obj.{{ field.name()|var_name }} as {{ field|type_name(ci) }})
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
