{%- import "CallBodyMacros.ts" as cb %}
{%- macro function(func) %}
{%- call cb::docstring(func.docstring) %}
export {% if func.is_async() %}async {% endif %}function {{ func.name }}(
    {%- call cb::arg_list_decl(func) -%}): {# space #}
    {%- call cb::return_type(func) %}
    {%- call cb::throws_kw(func) %} {
    {%- call cb::call_body_function(func) %}
    }
{%- endmacro %}
