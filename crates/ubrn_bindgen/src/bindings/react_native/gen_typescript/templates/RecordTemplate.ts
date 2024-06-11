{% include "RustBufferTemplate.ts" %}
{{ self.import_infra("uniffiCreateRecord", "records") }}

{%- let rec = ci|get_record_definition(name) %}
{%- call ts::docstring(rec, 0) %}
export type {{ type_name }} = {
    {%- for field in rec.fields() %}
    {%- call ts::docstring(field, 4) %}
    {{ field.name()|var_name }}: {{ field|type_name(ci) }}
    {%- if !loop.last %},{% endif %}
    {%- endfor %}
}

/**
 * Generated factory for {@link {{ type_name }}} record objects.
 */
export const {{ type_name }}Factory = (() => {
    const defaults = () => ({
        {%- for field in rec.fields() %}
        {%- match field.default_value() %}
        {%- when Some with(literal) %}
        {{- field.name()|var_name }}: {{ literal|render_literal(field, ci) }}
        {%- if !loop.last %},{% endif %}
        {%- else %}
        {%- endmatch -%}
        {%- endfor %}
    });
    return Object.freeze({
        /**
         * Create a frozen instance of {@link {{ type_name }}}, with defaults specified
         * in Rust, in the {@link {{ ci.namespace() }}} crate.
         */
        create: (() => {
            return uniffiCreateRecord<{{ type_name }}, ReturnType<typeof defaults>>(defaults);
        })(),

        /**
         * Defaults specified in the {@link {{ ci.namespace() }}} crate.
         */
        defaults: () => Object.freeze(defaults()) as Partial<{{ type_name }}>,
    });
})();

const {{ ffi_converter_name }} = (() => {
    type TypeName = {{ type_name }};
    class FFIConverter extends AbstractFfiConverterArrayBuffer<TypeName> {
        read(from: RustBuffer): TypeName {
            return {
            {%- for field in rec.fields() %}
                {{ field.name()|arg_name }}: {{ field|read_fn }}(from)
                {%- if !loop.last %}, {% endif %}
            {%- endfor %}
            };
        }
        write(value: TypeName, into: RustBuffer): void {
            {%- for field in rec.fields() %}
            {{ field|write_fn }}(value.{{ field.name()|var_name }}, into);
            {%- endfor %}
        }
        allocationSize(value: TypeName): number {
            {%- if rec.has_fields() %}
            return {% for field in rec.fields() -%}
                {{ field|allocation_size_fn }}(value.{{ field.name()|var_name }})
            {%- if !loop.last %} + {% else %};{% endif %}
            {% endfor %}
            {%- else %}
            return 0;
            {%- endif %}
        }
    };
    return new FFIConverter();
})();

{{- self.export_converter(ffi_converter_name) -}}
