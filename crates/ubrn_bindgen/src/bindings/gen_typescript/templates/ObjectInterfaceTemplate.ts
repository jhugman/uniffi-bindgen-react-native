{%- let protocol_docstring = obj.docstring() %}
{%- call ts::docstring_value(protocol_docstring, 0) %}
export interface {{ protocol_name }} {
    {% for meth in methods.iter() -%}
    {%- call ts::docstring(meth, 4) %}
    {{ meth.name()|fn_name }}({% call ts::arg_list_protocol(meth) %}) {% call ts::throws_kw(meth) -%}
    : {# space #}
    {%- call ts::return_type(meth) %};
    {%- endfor %}
}
