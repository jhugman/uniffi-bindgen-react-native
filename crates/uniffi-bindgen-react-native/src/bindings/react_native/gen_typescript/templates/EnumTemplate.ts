{{- self.add_import_from("FfiConverterRustBuffer", "ffi-converters") -}}
{{- self.add_import_from("FfiConverterInt32", "ffi-converters") -}}
{{- self.add_import_from("UniffiInternalError", "errors") -}}

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
    class FFIConverter extends FfiConverterRustBuffer<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (ordinalConverter.read(from)) {
                {%- for variant in e.variants() %}
                case {{ loop.index0 }}: return {{ type_name }}.{{ variant|variant_name }};
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        write(value: TypeName, into: RustBuffer): void {
            switch (value) {
                {%- for variant in e.variants() %}
                case {{ type_name }}.{{ variant|variant_name }}: return ordinalConverter.write({{ loop.index0 }}, into);
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
/// Typescript doesn't have the concept of enums with variant properties,
/// so we need to be a little creative here.
///
/// For a Rust enum:
/// ```rs
/// enum FilePath {
///   Local { path: String }
///   Remote { host: String, path: String }
/// }
/// ```
/// Currently:
/// ```ts
/// class FilePath {
///   static Local = class Local extends FilePath { constructor(public members: { path: string })} {}
///   static Remote = class Remote extends FilePath { constructor(public members: { host: string, path: string })} {}
/// }
/// ```
///
/// This gives nice construction properties:
/// ```ts
/// const path: FilePath = new FilePath.Local({ path: "/" });
/// ```
/// but not very nice pattern matching.
///
/// To help with switch statements, a companion enum (not ideal) called `FilePathKind` is generated:
///
/// ```ts
/// enum FilePathKind {
///   LOCAL,
///   REMOTE
/// }
/// ```
///
/// so:
/// ```ts
/// switch path.kind {
///     case FilePathKind.LOCAL:
///         console.log("It's a local file path");
///         break;
///     case FilePathKind.REMOTE:
///         console.log("It's a remote file path");
///         break;
/// }
/// ```
///
///
{%- let kind_type_name = format!("{type_name}Kind") %}
export enum {{ kind_type_name }} {
    {%- for variant in e.variants() %}
    {{ variant|variant_name }} = {{ loop.index0 }}
    {%- if !loop.last %},{% endif -%}
    {% endfor %}
}

{%- call ts::docstring(e, 0) %}
export abstract class {{ type_name }} {
    protected constructor(public kind: {{ kind_type_name }}) {}

    {%-   for variant in e.variants() %}
    {%-    call ts::docstring(variant, 4) %}
    {%-    let var_name = variant.name()|class_name(ci) %}
    static {{ var_name }} = class {{ var_name }} extends {{ type_name }} {
        constructor(
        {%- if !variant.fields().is_empty() %}
            public members: {
                {%- for field in variant.fields() %}
                {% call ts::field_name(field, loop.index) %}: {{ field|type_name(ci) }}
                {%- if loop.last -%}{%- else -%},{%- endif -%}{% endfor %}
            }
        {%- endif %}
        ) { super({{kind_type_name}}.{{ variant|variant_name }}); }
    }
    {%- endfor %}
}

const {{ ffi_converter_name }} = (() => {
    const ordinalConverter = FfiConverterInt32;
    type TypeName = {{ type_name }};
    class FFIConverter extends FfiConverterRustBuffer<TypeName> {
        read(from: RustBuffer): TypeName {
            switch (ordinalConverter.read(from)) {
                {%- for variant in e.variants() %}
                case {{ loop.index0 }}: return new {{ type_name }}.{{ variant.name()|class_name(ci) }}(
                {%- if !variant.fields().is_empty() %}{
                    {%- for field in variant.fields() %}
                    {% call ts::field_name(field, loop.index) %}: {{ field|read_fn }}(from)
                    {%- if loop.last -%}{%- else -%},{%- endif -%}{% endfor %}
                }
                {%- endif %}
                );
                {%- endfor %}
                default: throw new UniffiInternalError.UnexpectedEnumCase();
            }
        }
        write(value: TypeName, into: RustBuffer): void {
            switch (value.kind) {
                {%- for variant in e.variants() %}
                case {{ kind_type_name }}.{{ variant|variant_name }}: {
                    if (value instanceof {{ type_name }}.{{ variant.name()|class_name(ci) }}) {
                        ordinalConverter.write({{ loop.index0 }}, into);
                        {%- if !variant.fields().is_empty() %}
                        {%- for field in variant.fields() %}
                        {{ field|write_fn }}(value.members.{% call ts::field_name(field, loop.index) %}, into);
                        {%- endfor %}
                        {%- endif %}
                    }
                    break;
                }
                {%- endfor %}
                default: break;
            }
            throw new UniffiInternalError.UnexpectedEnumCase();
        }
        allocationSize(value: TypeName): number {
            switch (value.kind) {
                {%- for variant in e.variants() %}
                case {{ kind_type_name }}.{{ variant|variant_name }}: {
                    if (value instanceof {{ type_name }}.{{ variant.name()|class_name(ci) }}) {
                        {%- if !variant.fields().is_empty() %}
                        let size = ordinalConverter.allocationSize({{ loop.index0 }});
                        {%- for field in variant.fields() %}
                        size += {{ field|allocation_size_fn }}(value.members.{% call ts::field_name(field, loop.index) %});
                        {%- endfor %}
                        return size;
                        {%- else %}
                        return ordinalConverter.allocationSize({{ loop.index0 }});
                        {%- endif %}
                    }
                    break;
                }
                {%- endfor %}
                default: break;
            }
            throw new UniffiInternalError.UnexpectedEnumCase();
        }
    }
    return new FFIConverter();
})();
{%- endif %}{# endif enum.is_flat() #}

{#
We always write these public functions just in case the enum is used as
an external type by another crate.
#}
export function {{ ffi_converter_name }}_lift(buf: RustBuffer): {{ type_name }} {
    return {{ ffi_converter_name }}.lift(buf);
}

export function {{ ffi_converter_name }}_lower(value: {{ type_name }}): RustBuffer {
    return {{ ffi_converter_name }}.lower(value);
}
