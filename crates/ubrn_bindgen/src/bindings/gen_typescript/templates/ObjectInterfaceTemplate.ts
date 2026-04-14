{%- import "CallBodyMacros.ts" as cb %}
{%- macro object_interface(obj) %}
{%- if let Some(ds) = obj.docstring %}
{{ ds }}
{%- endif %}
export interface {{ obj.protocol_name }} {
    {% for meth in obj.methods -%}
    {%- if let Some(ds) = meth.docstring %}
{{ ds }}
    {%- endif %}
    {{ meth.name }}({% call cb::arg_list_protocol(meth) %}){% call cb::throws_kw(meth) %}: {% call cb::return_type(meth) %};
    {%- endfor %}
}
{%- endmacro %}
