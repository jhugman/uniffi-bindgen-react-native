{{- self.import_infra("uniffiCreateRecord", "records") }}

{%- let rec = ci.get_record_definition(name).expect("Record definition not found in this ci") %}
{%- let tm = rec.uniffi_trait_methods() %}
{%- call ts::docstring(rec, 0) %}
export type {{ type_name }} = {
    {%- for field in rec.fields() %}
    {%- call ts::docstring(field, 4) %}
    {{ field.name()|var_name }}: {{ field|type_name(self) }}
    {%- if !loop.last %},{% endif %}
    {%- endfor %}
}

/**
 * Generated factory for {@link {{ type_name }}} record objects.
 */
export const {{ decl_type_name }} = (() => {
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
    const create = (() => {
        return uniffiCreateRecord<{{ type_name }}, ReturnType<typeof defaults>>(defaults);
    })();
{%- let constructors = rec.constructors() %}
{%- let methods = rec.methods() %}
{%- let rust_has_create = rec.has_rust_constructor_named("create") %}
{%- let rust_has_new   = rec.has_rust_constructor_named("new") %}
    return Object.freeze({
        {%- if !rust_has_create %}
        create,
        {%- else %}
        // Note: Rust defines a constructor named 'create', replacing the default TypeScript factory helper.
        {%- endif %}
        {%- if !rust_has_new %}
        new: create,
        {%- endif %}
        defaults: () => Object.freeze(defaults()) as Partial<{{ type_name }}>,
{% call ts::uniffi_trait_methods_value_receiver(tm, ffi_converter_name, type_name, "    ", ",") %}
{%- if !constructors.is_empty() %}
{% call ts::value_receiver_constructors(constructors, "    ", ",") %}
{%- endif %}
{%- if !methods.is_empty() %}
{% call ts::value_receiver_methods(methods, ffi_converter_name, type_name, "    ", ",") %}
{%- endif %}
    });
})();

const {{ ffi_converter_name }} = (() => {
    type TypeName = {{ type_name }};
    {{- self.import_infra("AbstractFfiConverterByteArray", "ffi-converters") }}
    class FFIConverter extends AbstractFfiConverterByteArray<TypeName> {
        read(from: RustBuffer): TypeName {
            return {
            {%- for field in rec.fields() %}
                {{ field.name()|arg_name }}: {{ field|ffi_converter_name(self) }}.read(from)
                {%- if !loop.last %}, {% endif %}
            {%- endfor %}
            };
        }
        write(value: TypeName, into: RustBuffer): void {
            {%- for field in rec.fields() %}
            {{ field|ffi_converter_name(self) }}.write(value.{{ field.name()|var_name }}, into);
            {%- endfor %}
        }
        allocationSize(value: TypeName): number {
            {%- if rec.has_fields() %}
            return {% for field in rec.fields() -%}
                {{ field|ffi_converter_name(self) }}.allocationSize(value.{{ field.name()|var_name }})
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
