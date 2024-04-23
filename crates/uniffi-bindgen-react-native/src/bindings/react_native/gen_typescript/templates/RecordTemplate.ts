{% include "RustBufferTemplate.ts" %}
{%- let rec = ci|get_record_definition(name) %}
{%- call ts::docstring(rec, 0) %}
type {{ type_name }} = {
    {%- for field in rec.fields() %}
    {%- call ts::docstring(field, 4) %}
    {{ field.name()|var_name }}: {{ field|type_name(ci) }}
    {%- if !loop.last %},{% endif %}
    {%- endfor %}
}

// Default memberwise initializers are never public by default, so we
// declare one manually.
export function create{{ name }}({% call ts::field_list_decl(rec, false) %}) {
    return {
    {%- for field in rec.fields() %}
        {{ field.name()|var_name }}{% if !loop.last %},{% endif %}
    {%- endfor %}
    };
}

const {{ ffi_converter_name }} = (() => {
    type TypeName = {{ type_name }};
    class FFIConverter extends FfiConverterRustBuffer<TypeName> {
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

{#
We always write these public functions just in case the struct is used as
an external type by another crate.
#}
export function {{ ffi_converter_name }}_lift(buf: RustBuffer): {{ type_name }} {
    return {{ ffi_converter_name }}.lift(buf);
}

export function {{ ffi_converter_name }}_lower(value: {{ type_name }}): RustBuffer {
    return {{ ffi_converter_name }}.lower(value);
}
