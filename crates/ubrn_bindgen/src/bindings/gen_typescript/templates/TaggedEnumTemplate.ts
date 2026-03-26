{%- let type_name = e.ts_name %}
{%- let type_name__Tags = format!("{type_name}_Tags") %}

// {% if e.is_error %}Error type{% else %}Enum{% endif %}: {{ type_name }}
export enum {{ type_name__Tags }} {
    {%- for variant in e.variants %}
    {{ variant.name }} = "{{ variant.name }}"
    {%- if !loop.last %},{% endif -%}
    {% endfor %}
}

{%- if let Some(ds) = e.docstring %}
{{ ds }}
{%- endif %}
export const {{ type_name }} = (() => {
  {%- for variant in e.variants %}
    {%- let external_name = variant.name %}
    {%- let variant_class = format!("{external_name}_") %}
    {%- let variant_interface = format!("{variant_class}_interface") %}
    {%- let variant_tag = format!("{type_name__Tags}.{external_name}") %}
    {%- let has_fields = !variant.fields.is_empty() %}
    {%- let is_tuple = variant.has_nameless_fields %}

    type {{ variant_interface }} = {
        tag: {{ variant_tag }}
        {%- if has_fields %};
        inner: Readonly<{%- if !is_tuple %}{
{%-   for field in variant.fields %}
{{-     field.name }}: {{ field.ts_type }}
{%-     if !loop.last %}; {% endif -%}
{%- endfor %}}
{%- else %}
[
{%-   for field in variant.fields %}
{{-     field.ts_type }}
{%-     if !loop.last %}, {% endif -%}
{%- endfor %}
]
{%- endif %}>
        {%- endif %}
    };

    {%- if let Some(ds) = variant.docstring %}
{{ ds }}
    {%- endif %}
    class {{ variant_class }} extends {% if e.is_error %}UniffiError{% else %}UniffiEnum{% endif %} implements {{ variant_interface }} {
        /**
         * @private
         * This field is private and should not be used, use `tag` instead.
         */
        readonly [uniffiTypeNameSymbol] = "{{ type_name }}";
        readonly tag = {{ variant_tag }};
        {%- if has_fields %}
        readonly inner: Readonly<{%- if !is_tuple %}{
{%-   for field in variant.fields %}
{{-     field.name }}: {{ field.ts_type }}
{%-     if !loop.last %}; {% endif -%}
{%- endfor %}}
{%- else %}
[
{%-   for field in variant.fields %}
{{-     field.ts_type }}
{%-     if !loop.last %}, {% endif -%}
{%- endfor %}
]
{%- endif %}>;
        {%-   if !is_tuple %}
        constructor(inner: { {%- for field in variant.fields %}{{ field.name }}: {{ field.ts_type }}{%- if !loop.last %}; {% endif %}{%- endfor %} }) {
            super("{{ type_name }}", "{{ external_name }}");
            this.inner = Object.freeze(inner);
        }

        static new(inner: { {%- for field in variant.fields %}{{ field.name }}: {{ field.ts_type }}{%- if !loop.last %}; {% endif %}{%- endfor %} }): {{ variant_class }} {
            return new {{ variant_class }}(inner);
        }
        {%-   else %}
        constructor({%- for field in variant.fields %}v{{ loop.index0 }}: {{ field.ts_type }}{%- if !loop.last %}, {% endif %}{%- endfor -%}) {
            super("{{ type_name }}", "{{ external_name }}");
            this.inner = Object.freeze([{%- for field in variant.fields %}v{{ loop.index0 }}{%- if !loop.last %}, {% endif %}{%- endfor -%}]);
        }

        static new({%- for field in variant.fields %}v{{ loop.index0 }}: {{ field.ts_type }}{%- if !loop.last %}, {% endif %}{%- endfor -%}): {{ variant_class }} {
            return new {{ variant_class }}({%- for field in variant.fields %}v{{ loop.index0 }}{%- if !loop.last %}, {% endif %}{%- endfor -%});
        }
        {%-   endif %}
        {%- else %}
        constructor() {
            super("{{ type_name }}", "{{ external_name }}");
        }

        static new(): {{ variant_class }} {
            return new {{ variant_class }}();
        }
        {%- endif %}

        static instanceOf(obj: any): obj is {{ variant_class }} {
            return obj.tag === {{ variant_tag }};
        }

        {%- for ut in e.uniffi_traits %}
        {%- match ut %}
        {%- when TsUniffiTrait::Display { method } %}
        toString(): {% call cb::return_type(method) %} { const self_ = this as unknown as {{ type_name }};
            {% call cb::call_body_value(method) %}
        }
        {%- when TsUniffiTrait::Debug { method } %}
        toDebugString(): {% call cb::return_type(method) %} { const self_ = this as unknown as {{ type_name }};
            {% call cb::call_body_value(method) %}
        }
        {%- if !e.has_display_trait() %}
        toString(): {% call cb::return_type(method) %} { const self_ = this as unknown as {{ type_name }};
            {% call cb::call_body_value(method) %}
        }
        {%- endif %}
        {%- when TsUniffiTrait::Eq { eq, ne } %}
        equals(other: {{ type_name }}): {% call cb::return_type(eq) %} { const self_ = this as unknown as {{ type_name }};
            {% call cb::call_body_value(eq) %}
        }
        {%- when TsUniffiTrait::Hash { method } %}
        hashCode(): {% call cb::return_type(method) %} { const self_ = this as unknown as {{ type_name }};
            {% call cb::call_body_value(method) %}
        }
        {%- when TsUniffiTrait::Ord { cmp } %}
        compareTo(other: {{ type_name }}): {% call cb::return_type(cmp) %} { const self_ = this as unknown as {{ type_name }};
            {% call cb::call_body_value(cmp) %}
        }
        {%- endmatch %}
        {%- endfor %}

        {%- if e.is_error %}
        {%-   if has_fields %}
        static hasInner(obj: any): obj is {{ variant_class }} {
            return {{ variant_class }}.instanceOf(obj);
        }

        static getInner(obj: {{ variant_class }}): Readonly<{%- if !is_tuple %}{
{%-   for field in variant.fields %}
{{-     field.name }}: {{ field.ts_type }}
{%-     if !loop.last %}; {% endif -%}
{%- endfor %}}
{%- else %}
[
{%-   for field in variant.fields %}
{{-     field.ts_type }}
{%-     if !loop.last %}, {% endif -%}
{%- endfor %}
]
{%- endif %}> {
            return obj.inner;
        }
        {%- else %}
        static hasInner(obj: any): obj is {{ variant_class }} {
            return false;
        }
        {%-   endif %}
        {%- endif %}

    }
  {%- endfor %}

    function instanceOf(obj: any): obj is {{ type_name }} {
        return obj[uniffiTypeNameSymbol] === "{{ type_name }}";
    }

    return Object.freeze({
        instanceOf,
        {%- for cons in e.constructors %}
{% call cb::docstring(cons.docstring) %}
        {{ cons.name }}({% call cb::arg_list_decl(cons) %}): {% call cb::return_type(cons) %} {
{%- call cb::call_body_function(cons) %}
        },
        {%- endfor %}
        {%- for method in e.methods %}
{% call cb::docstring(method.docstring) %}
        {{ method.name }}(self_: {{ type_name }}{% if !method.arguments.is_empty() %}, {% endif %}{% call cb::arg_list_decl(method) %}): {% call cb::return_type(method) %} {
{%- call cb::call_body_value(method) %}
        },
        {%- endfor %}
  {%- for variant in e.variants %}
  {%-   let external_name = variant.name %}
  {%-   let variant_class = format!("{external_name}_") %}
  {{    external_name }}: {{ variant_class }}
  {%-   if !loop.last %}, {% endif -%}
  {%- endfor %}
    });

})();

{%- if let Some(ds) = e.docstring %}
{{ ds }}
{%- endif %}
export type {{ type_name }} = InstanceType<
    typeof {{ type_name }}[keyof Omit<typeof {{ type_name }}, 'instanceOf'>]
>;

// FfiConverter for enum {{ type_name }}
const {{ e.ffi_converter_name }} = (() => {
    const ordinalConverter = FfiConverterInt32;
    type TypeName = {{ type_name }};
    class FFIConverter extends AbstractFfiConverterByteArray<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (ordinalConverter.read(from)) {
            {%- for variant in e.variants %}
            {%-   let has_fields = !variant.fields.is_empty() %}
            {%-   let is_tuple = variant.has_nameless_fields %}
            {%-   let external_name = variant.name %}
                case {{ loop.index }}: return new {{ type_name }}.{{ external_name }}(
            {%-   if has_fields %}
            {%-     if !is_tuple %}{
            {%-     for field in variant.fields %}
            {{-       field.name }}: {{ field.ffi_converter }}.read(from)
            {%-       if !loop.last -%}, {% endif %}
            {%-     endfor %} }
            {%-     else %}
            {%-       for field in variant.fields %}
            {{-         field.ffi_converter }}.read(from)
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
                {%- for variant in e.variants %}
                {%-   let has_fields = !variant.fields.is_empty() %}
                {%-   let is_tuple = variant.has_nameless_fields %}
                {%-   let external_name = variant.name %}
                case {{ type_name__Tags }}.{{ external_name }}: {
                    ordinalConverter.write({{ loop.index }}, into);
                    {%- if has_fields %}
                    const inner = value.inner;
                    {%-   for field in variant.fields %}
                    {{ field.ffi_converter }}.write({%- if is_tuple %}inner[{{ loop.index0 }}]{% else %}inner.{{ field.name }}{% endif %}, into);
                    {%-   endfor %}
                    {%- endif %}
                    return;
                }
                {%- endfor %}
                default:
                    // Throwing from here means that {{ type_name__Tags }} hasn't matched an ordinal.
                    throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        allocationSize(value: TypeName): number {
            switch (value.tag) {
                {%- for variant in e.variants %}
                {%-   let has_fields = !variant.fields.is_empty() %}
                {%-   let is_tuple = variant.has_nameless_fields %}
                {%-   let external_name = variant.name %}
                case {{ type_name__Tags }}.{{ external_name }}: {
                {%-   if has_fields %}
                    const inner = value.inner;
                    let size = ordinalConverter.allocationSize({{ loop.index }});
                {%-     for field in variant.fields %}
                    size += {{ field.ffi_converter }}.allocationSize({%- if is_tuple %}inner[{{ loop.index0 }}]{% else %}inner.{{ field.name }}{% endif %});
                {%-     endfor %}
                    return size;
                {%-   else %}
                    return ordinalConverter.allocationSize({{ loop.index }});
                {%-   endif %}
                }
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
    }
    return new FFIConverter();
})();
