{%- if config.use_codegen %}
{%- include "wrapper-with-codegen.ts" %}
{%- else %}
{%- include "wrapper-without-codegen.ts" %}
{%- endif %}
