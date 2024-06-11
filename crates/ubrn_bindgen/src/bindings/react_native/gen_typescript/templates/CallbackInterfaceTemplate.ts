{%- let cbi = ci|get_callback_interface_definition(name) %}
{%- let callback_handler = format!("uniffiCallbackHandler{}", name) %}
{%- let callback_init = format!("uniffiCallbackInit{}", name) %}
{%- let methods = cbi.methods() %}
{%- let protocol_name = type_name.clone() %}
{%- let protocol_docstring = cbi.docstring() %}
{%- let vtable = cbi.vtable() %}
{%- let vtable_methods = cbi.vtable_methods() %}
{%- let ffi_init_callback = cbi.ffi_init_callback() %}
{{- self.import_infra("UniffiHandleMap", "handle-map") }}
{{- self.import_infra_type("UniffiHandle", "handle-map") }}
{{- self.import_infra_type("FfiConverter", "ffi-converters") }}
{{- self.import_infra("FfiConverterUInt64", "ffi-converters") }}
{#- obj is used to generate an interface with ObjectInterfaceTemplate.ts #}
{%- let obj = cbi %}
{% include "ObjectInterfaceTemplate.ts" %}
{% include "CallbackInterfaceImpl.ts" %}

// FfiConverter protocol for callback interfaces
const {{ ffi_converter_name }} = (() => {
    type TypeName = {{ type_name }};
    const handleConverter = FfiConverterUInt64;
    const handleMap = new UniffiHandleMap<TypeName>();
    class FFIConverter implements FfiConverter<UniffiHandle, TypeName> {
        lift(value: UniffiHandle): TypeName {
            return handleMap.get(value);
        }
        lower(value: TypeName): UniffiHandle {
            return handleMap.insert(value);
        }
        read(from: RustBuffer): TypeName  {
            return this.lift(handleConverter.read(from));
        }
        write(value: TypeName, into: RustBuffer): void {
            handleConverter.write(this.lower(value), into);
        }
        allocationSize(value: TypeName): number {
            return handleConverter.allocationSize(BigInt("0"));
        }
    }
    return new FFIConverter();
})();
