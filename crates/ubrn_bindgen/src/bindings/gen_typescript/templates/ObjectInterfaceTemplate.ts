{%- if let Some(ds) = obj.docstring %}
{{ ds }}
{%- endif %}
export interface {{ obj.protocol_name }} {
    {% for meth in obj.methods -%}
    {%- if let Some(ds) = meth.docstring %}
{{ ds }}
    {%- endif %}
    {{ meth.name }}(
    {%- for arg in meth.arguments -%}
        {{ arg.name }}: {{ arg.ts_type }}
        {%- if !loop.last %}, {% endif -%}
    {%- endfor -%}
    {%- if meth.is_async() -%}
    {%-   if !meth.arguments.is_empty() %}, {% endif -%}
    asyncOpts_?: { signal: AbortSignal }
    {%- endif -%}
    ) {% if meth.is_throwing() %}/*throws*/{% endif -%}
    : {# space #}
    {%- if meth.is_async() %}Promise<
    {%- match meth.return_type -%}
    {%- when Some with (rt) %}{{ rt.ts_type }}
    {%- when None %}void
    {%- endmatch %}>
    {%- else %}
    {%- match meth.return_type -%}
    {%- when Some with (rt) %}{{ rt.ts_type }}
    {%- when None %}void
    {%- endmatch %}
    {%- endif %};
    {%- endfor %}
}
