{%- let protocol_docstring = obj.docstring() %}
{%- call ts::docstring_value(protocol_docstring, 0) %}
export interface {{ protocol_name }} {
    {% for meth in methods.iter() -%}
    {%- call ts::docstring(meth, 4) %}
    {% call ts::async(meth) -%}{{ meth.name()|fn_name }}({% call ts::arg_list_protocol(meth) %}) {% call ts::throws(meth) -%}
    {%- match meth.return_type() -%}
    {%- when Some with (return_type) %}: {{ return_type|type_name(ci) -}}
    {%- else -%}
    {%- endmatch %};
    {%- endfor %}
}
