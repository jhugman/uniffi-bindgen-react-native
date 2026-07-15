{%- import "CallBodyMacros.ts" as cb %}
{%- macro record(rec) %}
{%- if let Some(ds) = rec.docstring %}
{{ ds }}
{%- endif %}
export type {{ rec.ts_name }} = {
    {%- for field in rec.fields %}
    {%- if let Some(ds) = field.docstring %}
{{ ds }}
    {%- endif %}
    {% call cb::field_decl(field) %}{% if !loop.last %},{% endif %}
    {%- endfor %}
}

/**
 * Generated factory for {@link {{ rec.ts_name }}} record objects.
 */
export const {{ rec.ts_name }} = (() => {
    const defaults = () => ({
        {%- for field in rec.fields %}
        {%- if let Some(dv) = field.default_value %}
        {{ field.name }}: {{ dv }}
        {%- if !loop.last %},{% endif %}
        {%- endif %}
        {%- endfor %}
    });
    const create = (() => {
        return uniffiCreateRecord<{{ rec.ts_name }}, ReturnType<typeof defaults>>(defaults);
    })();
    return Object.freeze({
        {%- if !rec.has_create_constructor %}
        create,
        {%- else %}
        // Note: Rust defines a constructor named 'create', replacing the default TypeScript factory helper.
        {%- endif %}
        {%- if !rec.has_new_constructor %}
        new: create,
        {%- endif %}
        defaults: () => Object.freeze(defaults()) as Partial<{{ rec.ts_name }}>,
{%- for ut in rec.uniffi_traits %}
{%- match ut %}
{%- when TsUniffiTrait::Display { method } %}
        {% if method.renders_async() %}async {% endif %}{% if method.renders_async() %}asyncToString{% else %}toString{% endif %}(self_: {{ rec.ts_name }}): {% call cb::return_type(method) %} {
        {% call cb::call_body_value(method) %}
        },
{%- when TsUniffiTrait::Debug { method } %}
        {% if method.renders_async() %}async {% endif %}toDebugString(self_: {{ rec.ts_name }}): {% call cb::return_type(method) %} {
        {% call cb::call_body_value(method) %}
        },
{%- if !rec.has_display_trait() %}
        {% if method.renders_async() %}async {% endif %}{% if method.renders_async() %}asyncToString{% else %}toString{% endif %}(self_: {{ rec.ts_name }}): {% call cb::return_type(method) %} {
        {% call cb::call_body_value(method) %}
        },
{%- endif %}
{%- when TsUniffiTrait::Eq { eq, ne } %}
        {% if eq.renders_async() %}async {% endif %}equals(self_: {{ rec.ts_name }}, {% call cb::arg_list_decl(eq) %}): {% call cb::return_type(eq) %} {
        {% call cb::call_body_value(eq) %}
        },
{%- when TsUniffiTrait::Hash { method } %}
        {% if method.renders_async() %}async {% endif %}hashCode(self_: {{ rec.ts_name }}): {% call cb::return_type(method) %} {
        {% call cb::call_body_value(method) %}
        },
{%- when TsUniffiTrait::Ord { cmp } %}
        {% if cmp.renders_async() %}async {% endif %}compareTo(self_: {{ rec.ts_name }}, {% call cb::arg_list_decl(cmp) %}): {% call cb::return_type(cmp) %} {
        {% call cb::call_body_value(cmp) %}
        },
{%- endmatch %}
{%- endfor %}
{%- for cons in rec.constructors %}
        {% if cons.renders_async() %}async {% endif %}{{ cons.name }}({% call cb::arg_list_decl(cons) %}): {% call cb::return_type(cons) %} {
{%- call cb::call_body_function(cons) %}
        },
{%- endfor %}
{%- for method in rec.methods %}
        {% if method.renders_async() %}async {% endif %}{{ method.name }}(self_: {{ rec.ts_name }}{% if !method.arguments.is_empty() %}, {% endif %}{% call cb::arg_list_decl(method) %}): {% call cb::return_type(method) %} {
{%- call cb::call_body_value(method) %}
        },
{%- endfor %}
    });
})();

const {{ rec.ffi_converter_name }} = (() => {
    type TypeName = {{ rec.ts_name }};
    class FFIConverter extends AbstractFfiConverterByteArray<TypeName> {
        read(from: RustBuffer): TypeName {
            return {
            {%- for field in rec.fields %}
                {{ field.name }}: {{ field.ffi_converter }}.read(from)
                {%- if !loop.last %}, {% endif %}
            {%- endfor %}
            };
        }
        write(value: TypeName, into: RustBuffer): void {
            {%- for field in rec.fields %}
            {{ field.ffi_converter }}.write(value.{{ field.name }}, into);
            {%- endfor %}
        }
        allocationSize(value: TypeName): number {
            {%- if rec.fields.is_empty() %}
            return 0;
            {%- else %}
            return {%- for field in rec.fields %} {{ field.ffi_converter }}.allocationSize(value.{{ field.name }})
            {%- if !loop.last %} +{% else %};{% endif %}
            {% endfor %}
            {%- endif %}
        }
    };
    return new FFIConverter();
})();
{%- endmacro %}
