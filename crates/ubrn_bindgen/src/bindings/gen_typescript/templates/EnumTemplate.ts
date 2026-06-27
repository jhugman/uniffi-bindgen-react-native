{%- import "CallBodyMacros.ts" as cb %}
{%- macro flat_enum(e) %}
{%- if let Some(ds) = e.docstring %}
{{ ds }}
{%- endif %}
export enum {{ e.ts_name }} {
    {%- for variant in e.variants %}
    {%- if let Some(ds) = variant.docstring %}
{{ ds }}
    {%- endif %}
    {{ variant.name }}
    {%- if e.discr_type.is_some() %} = {{ variant.discriminant }}
    {%- endif %}
    {%- if !loop.last %},{% endif -%}
    {% endfor %}
}
{%- if e.has_callables() %}

export namespace {{ e.ts_name }} {
{%- for ut in e.uniffi_traits %}
{%- match ut %}
{%- when TsUniffiTrait::Display { method } %}
    export function toString(self_: {{ e.ts_name }}): {% call cb::return_type(method) %} {
        {% call cb::call_body_value(method) %}
    }
{%- when TsUniffiTrait::Debug { method } %}
    export function toDebugString(self_: {{ e.ts_name }}): {% call cb::return_type(method) %} {
        {% call cb::call_body_value(method) %}
    }
{%- if !e.has_display_trait() %}
    export function toString(self_: {{ e.ts_name }}): {% call cb::return_type(method) %} {
        {% call cb::call_body_value(method) %}
    }
{%- endif %}
{%- when TsUniffiTrait::Eq { eq, ne } %}
    export function equals(self_: {{ e.ts_name }}, {% call cb::arg_list_decl(eq) %}): {% call cb::return_type(eq) %} {
        {% call cb::call_body_value(eq) %}
    }
{%- when TsUniffiTrait::Hash { method } %}
    export function hashCode(self_: {{ e.ts_name }}): {% call cb::return_type(method) %} {
        {% call cb::call_body_value(method) %}
    }
{%- when TsUniffiTrait::Ord { cmp } %}
    export function compareTo(self_: {{ e.ts_name }}, {% call cb::arg_list_decl(cmp) %}): {% call cb::return_type(cmp) %} {
        {% call cb::call_body_value(cmp) %}
    }
{%- endmatch %}
{%- endfor %}
{%- for cons in e.constructors %}
    export function {{ cons.name }}({% call cb::arg_list_decl(cons) %}): {% call cb::return_type(cons) %} {
{%- call cb::call_body_function(cons) %}
    }
{%- endfor %}
{%- for method in e.methods %}
    export function {{ method.name }}(self_: {{ e.ts_name }}{% if !method.arguments.is_empty() %}, {% endif %}{% call cb::arg_list_decl(method) %}): {% call cb::return_type(method) %} {
{%- call cb::call_body_value(method) %}
    }
{%- endfor %}
}
{%- endif %}

const {{ e.ffi_converter_name }} = (() => {
    const ordinalConverter = FfiConverterInt32;
    type TypeName = {{ e.ts_name }};
    class FFIConverter extends AbstractFfiConverterByteArray<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (ordinalConverter.read(from)) {
                {%- for variant in e.variants %}
                case {{ loop.index }}: return {{ e.ts_name }}.{{ variant.name }};
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        write(value: TypeName, into: RustBuffer): void {
            switch (value) {
                {%- for variant in e.variants %}
                case {{ e.ts_name }}.{{ variant.name }}: return ordinalConverter.write({{ loop.index }}, into);
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        allocationSize(value: TypeName): number {
            switch (value) {
                {%- for variant in e.variants() %}
                case {{ type_name }}.{{ variant|variant_name }}: return ordinalConverter.allocationSize(0);
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
    }
    return new FFIConverter();
})();
{%- endmacro %}
