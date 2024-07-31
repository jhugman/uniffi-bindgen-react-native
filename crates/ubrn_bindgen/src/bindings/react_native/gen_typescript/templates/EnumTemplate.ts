{{- self.import_infra("AbstractFfiConverterArrayBuffer", "ffi-converters") -}}
{{- self.import_infra("FfiConverterInt32", "ffi-converters") -}}
{{- self.import_infra("UniffiInternalError", "errors") -}}

{% if e.is_flat() %}
{%- call ts::docstring(e, 0) %}
export enum {{ type_name }} {
    {%- for variant in e.variants() %}
    {%- call ts::docstring(variant, 4) %}
    {{ variant|variant_name }}
    {%- match e.variant_discr_type() %}
    {%- when Some with (_) %} = {{ e|variant_discr_literal(loop.index0, ci) }}
    {%- else %}{% endmatch %}
    {%- if !loop.last %},{% endif -%}
    {% endfor %}
}

const {{ ffi_converter_name }} = (() => {
    const ordinalConverter = FfiConverterInt32;
    type TypeName = {{ type_name }};
    class FFIConverter extends AbstractFfiConverterArrayBuffer<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (ordinalConverter.read(from)) {
                {%- for variant in e.variants() %}
                case {{ loop.index0 + 1}}: return {{ type_name }}.{{ variant|variant_name }};
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        write(value: TypeName, into: RustBuffer): void {
            switch (value) {
                {%- for variant in e.variants() %}
                case {{ type_name }}.{{ variant|variant_name }}: return ordinalConverter.write({{ loop.index0 + 1 }}, into);
                {%- endfor %}
            }
        }
        allocationSize(value: TypeName): number {
            return ordinalConverter.allocationSize(0);
        }
    }
    return new FFIConverter();
})();

{% else %}

// Enum: {{ type_name }}
{%- let decl_type_name = e|decl_type_name(ci) %}
{%- let kind_type_name = format!("{type_name}_Tags") %}
export enum {{ kind_type_name }} {
    {%- for variant in e.variants() %}
    {{ variant|variant_name }} = "{{ variant.name() }}"
    {%- if !loop.last %},{% endif -%}
    {% endfor %}
}

{%- call ts::docstring(e, 0) %}
export const {{ decl_type_name }} = (() => {
  {%- for variant in e.variants() %}
    {%- let variant_name = variant.name()|class_name(ci) %}
    {%- let variant_data = variant_name|fmt("{}_data") %}
    {%- let variant_interface = variant_name|fmt("{}_interface") %}
    {%- let variant_tag = format!("{kind_type_name}.{variant_name}") %}
    {%- let has_fields = !variant.fields().is_empty() %}
    {%- let is_tuple = variant.has_nameless_fields() %}
    {%- if has_fields %}
    type {{ variant_data }} = {# space #}
    {%-   if !is_tuple %}{
    {%-     for field in variant.fields() %}
    {{-       field.name()|var_name }}: {{ field|type_name(ci) }}
    {%-       if !loop.last %}; {% endif -%}
    {%-     endfor %}}
    {%-   else %}[
    {%-     for field in variant.fields() %}
    {{-       field|type_name(ci) }}
    {%-       if !loop.last %}, {% endif -%}
    {%-     endfor %}]
    {%-   endif %};
    {%- endif %}
    type {{ variant_interface }} = { tag: {{ variant_tag }} {%- if has_fields %}; data: Readonly<{{ variant_data }}> {%- endif %}};

    {% call ts::docstring(variant, 4) %}
    class {{ variant_name }} implements {{ variant_interface }} {
        readonly tag = {{ variant_tag }};
        {%- if has_fields %}
        readonly data: Readonly<{{ variant_data }}>;
        {%-   if !is_tuple %}
        constructor(data: { {% call ts::field_list_decl(variant, false) %} }) {
            this.data = Object.freeze(data);
        }

        static new(data: { {% call ts::field_list_decl(variant, false) %} }): {{ variant_interface }} {
            return new {{ variant_name }}(data);
        }
        {%-   else %}
        constructor({%- call ts::field_list_decl(variant, true) -%}) {
            this.data = Object.freeze([{%- call ts::field_list(variant, true) -%}]);
        }

        static new({%- call ts::field_list_decl(variant, true) -%}): {{ variant_name }} {
            return new {{ variant_name }}({%- call ts::field_list(variant, true) -%});
        }
        {%-   endif %}
        {%- else %}
        constructor() {
            // NOOP
        }

        static new(): {{ variant_name }} {
            return new {{ variant_name }}();
        }
        {%- endif %}

        static instanceOf(obj: any): obj is {{ variant_interface }} {
            return obj.tag === {{ variant_tag }};
        }
    }


  {%- endfor %}

    function instanceOf(obj: any): obj is {# space #}
  {%- for variant in e.variants() %}
  {{-  variant.name()|class_name(ci)|fmt("{}_interface") }}
  {%-  if !loop.last %}| {% endif -%}
  {%- endfor %} {
        return obj.tag !== undefined && {{ kind_type_name }}.hasOwnProperty(obj.tag);
    }

    return {
        instanceOf,
  {%- for variant in e.variants() %}
  {{    variant.name()|class_name(ci) }}
  {%-   if !loop.last %}, {% endif -%}
  {%- endfor %}
    };

})();

{% call ts::docstring(e, 0) %}
{% call ts::type_omit_instanceof(type_name, decl_type_name) %}

// FfiConverter for enum {{ type_name }}
const {{ ffi_converter_name }} = (() => {
    const ordinalConverter = FfiConverterInt32;
    type TypeName = {{ type_name }};
    class FFIConverter extends AbstractFfiConverterArrayBuffer<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (ordinalConverter.read(from)) {
            {%- for variant in e.variants() %}
                case {{ loop.index }}: return new {{ type_name }}.{{ variant|variant_name }}(
            {%-   if !variant.fields().is_empty() %}
            {%-     if !variant.has_nameless_fields() %}{
            {%-     for field in variant.fields() %}
            {{-       field.name()|var_name }}: {{ field|read_fn }}(from)
            {%-       if !loop.last -%}, {% endif %}
            {%-     endfor %} }
            {%-     else %}
            {%-       for field in variant.fields() %}
            {{-         field|read_fn }}(from)
            {%-         if !loop.last -%}, {% endif %}
            {%-       endfor %}
            {%-     endif %}
            {%-   endif %});
            {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        write(value: TypeName, into: RustBuffer): void {
            switch (value.tag) {
                {%- for variant in e.variants() %}
                case {{ kind_type_name }}.{{ variant|variant_name }}: {
                    ordinalConverter.write({{ loop.index }}, into);
                    {%- if !variant.fields().is_empty() %}
                    const inner = value.data;
                    {%-   for field in variant.fields() %}
                    {{ field|write_fn }}({% call ts::field_name("inner", field, loop.index0) %}, into);
                    {%-   endfor %}
                    {%- endif %}
                    return;
                }
                {%- endfor %}
                default:
                    // Throwing from here means that {{ kind_type_name }} hasn't matched an ordinal.
                    throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        allocationSize(value: TypeName): number {
            switch (value.tag) {
                {%- for variant in e.variants() %}
                case {{ kind_type_name }}.{{ variant|variant_name }}: {
                    {%- if !variant.fields().is_empty() %}
                    const inner = value.data;
                    let size = ordinalConverter.allocationSize({{ loop.index }});
                    {%- for field in variant.fields() %}
                    size += {{ field|allocation_size_fn }}({% call ts::field_name("inner", field, loop.index0) %});
                    {%- endfor %}
                    return size;
                    {%- else %}
                    return ordinalConverter.allocationSize({{ loop.index }});
                    {%- endif %}
                }
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
    }
    return new FFIConverter();
})();
{%- endif %}{# endif enum.is_flat() #}

{{- self.export_converter(ffi_converter_name) -}}
