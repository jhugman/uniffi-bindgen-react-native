{%- macro custom_type(custom) %}
{%- match custom.custom_config %}
{%-   when None %}
{#- No config, just forward all methods to our builtin type #}
/**
 * Typealias from the type name used in the UDL file to the builtin type.  This
 * is needed because the UDL type name is used in function/method signatures.
 */
export type {{ custom.type_name }} = {{ custom.builtin_type_name }};
// FfiConverter for {{ custom.type_name }}, a type alias for {{ custom.builtin_type_name }}.
const {{ custom.ffi_converter_name }} = {{ custom.builtin_ffi_converter }};

{%-   when Some with (config) %}

{#- When the config specifies a different type name, create a typealias for it #}
{%-     match config.concrete_type_name %}
{%-       when Some with (concrete_type_name) %}
/**
 * Typealias from the type name used in the UDL file to the custom type.  This
 * is needed because the UDL type name is used in function/method signatures.
 */
export type {{ custom.type_name }} = {{ concrete_type_name }};
{%-       else %}
{%-     endmatch %}

// FfiConverter for {{ custom.type_name }}
const {{ custom.ffi_converter_name }} = (() => {
    type TsType = {{ custom.type_name }};
    type FfiType = {{ custom.ffi_type_name }};
    const intermediateConverter = {{ custom.builtin_ffi_converter }};
    class FFIConverter implements FfiConverter<FfiType, TsType> {
        lift(value: FfiType): TsType {
            const intermediate = intermediateConverter.lift(value);
            return {{ config.lift_expr }};
        }
        lower(value: TsType): FfiType {
            const intermediate = {{ config.lower_expr }};
            return intermediateConverter.lower(intermediate);
        }
        read(from: RustBuffer): TsType {
            const intermediate = intermediateConverter.read(from);
            return {{ config.lift_expr }};
        }
        write(value: TsType, into: RustBuffer): void {
            const intermediate = {{ config.lower_expr }};
            intermediateConverter.write(intermediate, into);
        }
        allocationSize(value: TsType): number {
            const intermediate = {{ config.lower_expr }};
            return intermediateConverter.allocationSize(intermediate);
        }
    }

    return new FFIConverter();
})();
{%- endmatch %}
{%- endmacro %}