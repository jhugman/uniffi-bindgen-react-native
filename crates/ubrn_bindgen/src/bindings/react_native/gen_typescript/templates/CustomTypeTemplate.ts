{%- let ffi_type_name=builtin|ffi_type|ffi_type_name %}
{%- match config.custom_types.get(name.as_str())  %}
{%-   when None %}
{#- No config, just forward all methods to our builtin type #}
/**
 * Typealias from the type name used in the UDL file to the builtin type.  This
 * is needed because the UDL type name is used in function/method signatures.
 */
export type {{ type_name }} = {{ builtin|type_name(ci) }};
// FfiConverter for {{ type_name }}, a type alias for {{ builtin|type_name(ci) }}.
const {{ ffi_converter_name }} = {{ builtin|ffi_converter_name }};

{%-   when Some with (config) %}

{# When the config specifies a different type name, create a typealias for it #}
{%-     match config.type_name %}
{%-       when Some with (concrete_type_name) %}
/**
 * Typealias from the type name used in the UDL file to the custom type.  This
 * is needed because the UDL type name is used in function/method signatures.
 */
export type {{ type_name }} = {{ concrete_type_name }};
{%-       else %}
{%-     endmatch %}

{%-     for (what, from) in config.imports %}
{{        self.import_custom(what, from) }}
{%-     endfor %}
{{- self.import_infra_type("FfiConverter", "ffi-converters") }}

// FfiConverter for {{ type_name }}
const {{ ffi_converter_name }} = (() => {
    type TsType = {{ type_name }};
    type FfiType = {{ ffi_type_name }};
    const intermediateConverter = {{ builtin|ffi_converter_name }};
    class FFIConverter implements FfiConverter<FfiType, TsType> {
        lift(value: FfiType): TsType {
            const intermediate = intermediateConverter.lift(value);
            return {{ config.into_custom.render("intermediate") }};
        }
        lower(value: TsType): FfiType {
            const intermediate = {{ config.from_custom.render("value") }};
            return intermediateConverter.lower(intermediate);
        }
        read(from: RustBuffer): TsType {
            const intermediate = intermediateConverter.read(from);
            return {{ config.into_custom.render("intermediate") }};
        }
        write(value: TsType, into: RustBuffer): void {
            const intermediate = {{ config.from_custom.render("value") }};
            intermediateConverter.write(intermediate, into);
        }
        allocationSize(value: TsType): number {
            const intermediate = {{ config.from_custom.render("value") }};
            return intermediateConverter.allocationSize(intermediate);
        }
    }

    return new FFIConverter();
})();
{%- endmatch %}

{{- self.export_converter(ffi_converter_name) -}}
