{{- self.import_infra("uniffiTypeNameSymbol", "symbols") -}}

// Enum: {{ type_name }}
{%- let type_name__Tags = format!("{type_name}_Tags") %}
export enum {{ type_name__Tags }} {
    {%- for variant in e.variants() %}
    {{ variant|variant_name }} = "{{ variant.name() }}"
    {%- if !loop.last %},{% endif -%}
    {% endfor %}
}

{%- macro variant_data_type(variant) %}Readonly<
{%-   let fields = variant.fields() %}
{%-   if !is_tuple %}{
{%-     for field in fields %}
{{-       field.name()|var_name }}: {{ field|type_name(self) }}
{%-       if !loop.last %}; {% endif -%}
{%-     endfor %}}
{%-   else %}
[
{%-     for field in fields %}
{{-       field|type_name(self) }}
{%-       if !loop.last %}, {% endif -%}
{%-     endfor %}
]
{%-   endif %}>
{%- endmacro %}

{%- call ts::docstring(e, 0) %}
export const {{ decl_type_name }} = (() => {
  {%- for variant in e.variants() %}
    {#
    // We have an external name and an internal name so that variants of the enum can
    // have names the same as other types. Since we're building Tagged Enums from
    // scratch in Typescript, we should make effort to make enums match what is possible
    // or common in Rust.
    // Internally to this IIFE, we use a generated name which is impossible to express
    // in a different type name in Rust (we append a `_`, which would be sifted out by
    // UpperCamelCasing of type names into Typescript). This lets use the class without
    // colliding with the outside world. e.g.  `new Variant_(record: Variant)`.
    // Externally, just as we return, we name it with the Variant name, so now
    // client code can use `new MyEnum.Variant(record: Variant)`.
    #}
    {%- let external_name = variant.name()|class_name(ci) %}
    {%- let variant_name = external_name|fmt("{}_") %}
    {%- let variant_interface = variant_name|fmt("{}_interface") %}
    {%- let variant_tag = format!("{type_name__Tags}.{external_name}") %}
    {%- let has_fields = !variant.fields().is_empty() %}
    {%- let is_tuple = variant.has_nameless_fields() %}

    type {{ variant_interface }} = {
        tag: {{ variant_tag }}
        {%- if has_fields %};
        inner: {% call variant_data_type(variant) %}
        {%- endif %}
    };

    {% call ts::docstring(variant, 4) %}
    class {{ variant_name }} extends {{ superclass }} implements {{ variant_interface }} {
        /**
         * @private
         * This field is private and should not be used, use `tag` instead.
         */
        readonly [uniffiTypeNameSymbol] = "{{ type_name }}";
        readonly tag = {{ variant_tag }};
        {%- if has_fields %}
        readonly inner: {% call variant_data_type(variant) %};
        {%-   if !is_tuple %}
        constructor(inner: { {% call ts::field_list_decl(variant, false) %} }) {
            super("{{ type_name }}", "{{ external_name }}");
            this.inner = Object.freeze(inner);
        }

        static new(inner: { {% call ts::field_list_decl(variant, false) %} }): {{ variant_name }} {
            return new {{ variant_name }}(inner);
        }
        {%-   else %}
        constructor({%- call ts::field_list_decl(variant, true) -%}) {
            super("{{ type_name }}", "{{ external_name }}");
            this.inner = Object.freeze([{%- call ts::field_list(variant, true) -%}]);
        }

        static new({%- call ts::field_list_decl(variant, true) -%}): {{ variant_name }} {
            return new {{ variant_name }}({%- call ts::field_list(variant, true) -%});
        }
        {%-   endif %}
        {%- else %}
        constructor() {
            super("{{ type_name }}", "{{ external_name }}");
        }

        static new(): {{ variant_name }} {
            return new {{ variant_name }}();
        }
        {%- endif %}

        static instanceOf(obj: any): obj is {{ variant_name }} {
            return obj.tag === {{ variant_tag }};
        }

        {% if is_error %}
        {%-   if has_fields %}
        static hasInner(obj: any): obj is {{ variant_name }} {
            return {{ variant_name }}.instanceOf(obj);
        }

        static getInner(obj: {{ variant_name }}): {% call variant_data_type(variant) %} {
            return obj.inner;
        }
        {%- else %}
        static hasInner(obj: any): obj is {{ variant_name }} {
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
  {%- for variant in e.variants() %}
  {%-   let external_name = variant.name()|class_name(ci) %}
  {%-   let variant_name = external_name|fmt("{}_") %}
  {{    external_name }}: {{ variant_name }}
  {%-   if !loop.last %}, {% endif -%}
  {%- endfor %}
    });

})();

{% call ts::docstring(e, 0) %}
{% call ts::type_omit_instanceof(type_name, decl_type_name) %}

// FfiConverter for enum {{ type_name }}
const {{ ffi_converter_name }} = (() => {
    const ordinalConverter = FfiConverterInt32;
    type TypeName = {{ type_name }};
    {{- self.import_infra("AbstractFfiConverterByteArray", "ffi-converters") }}
    class FFIConverter extends AbstractFfiConverterByteArray<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (ordinalConverter.read(from)) {
            {%- for variant in e.variants() %}
            {%-   let has_fields = !variant.fields().is_empty() %}
            {%-   let is_tuple = variant.has_nameless_fields() %}
                case {{ loop.index }}: return new {{ type_name }}.{{ variant|variant_name }}(
            {%-   if has_fields %}
            {%-     if !is_tuple %}{
            {%-     for field in variant.fields() %}
            {{-       field.name()|var_name }}: {{ field|ffi_converter_name(self) }}.read(from)
            {%-       if !loop.last -%}, {% endif %}
            {%-     endfor %} }
            {%-     else %}
            {%-       for field in variant.fields() %}
            {{-         field|ffi_converter_name(self) }}.read(from)
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
                {%-   let has_fields = !variant.fields().is_empty() %}
                {%-   let is_tuple = variant.has_nameless_fields() %}
                case {{ type_name__Tags }}.{{ variant|variant_name }}: {
                    ordinalConverter.write({{ loop.index }}, into);
                    {%- if has_fields %}
                    const inner = value.inner;
                    {%-   for field in variant.fields() %}
                    {{ field|ffi_converter_name(self) }}.write({% call ts::field_name("inner", field, loop.index0) %}, into);
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
                {%- for variant in e.variants() %}
                {%-   let has_fields = !variant.fields().is_empty() %}
                {%-   let is_tuple = variant.has_nameless_fields() %}
                case {{ type_name__Tags }}.{{ variant|variant_name }}: {
                {%-   if has_fields %}
                    const inner = value.inner;
                    let size = ordinalConverter.allocationSize({{ loop.index }});
                {%-     for field in variant.fields() %}
                    size += {{ field|ffi_converter_name(self) }}.allocationSize({% call ts::field_name("inner", field, loop.index0) %});
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
