{%- if self.include_once_check("ObjectRuntime.ts") %}
{%- include "ObjectRuntime.ts" %}
{%- endif %}
{%- let obj = ci|get_object_definition(name) %}
{%- let (protocol_name, impl_class_name) = obj|object_names(ci) %}
{%- let methods = obj.methods() %}

{%- let is_error = ci.is_name_used_as_error(name) %}

{%- include "ObjectInterfaceTemplate.ts" %}

{% call ts::docstring(obj, 0) %}
export class {{ impl_class_name }} implements {{ protocol_name }}, UniffiObjectInterface
    {%- for tm in obj.uniffi_traits() %}
    {%-     match tm %}
    {%-         when UniffiTrait::Display { fmt } %}, CustomStringConvertible
    {%-         when UniffiTrait::Debug { fmt } %}, CustomDebugStringConvertible
    {%-         when UniffiTrait::Eq { eq, ne } %}, Equatable
    {%-         when UniffiTrait::Hash { hash } %}, Hashable
    {%-         else %}
    {%-    endmatch %}
    {%- endfor %}
    {%- if is_error %}, Error{% endif %} {

    private pointer: bigint;

    {%- match obj.primary_constructor() %}
    {%- when Some with (cons) %}
    {%- call ts::ctor_decl(cons, 4) %}
    {%- when None %}
    // No primary constructor declared for this class.
    {%- endmatch %}

    {% for cons in obj.alternate_constructors() %}
    {%- call ts::func_decl("public static", cons, 4, "") %}
    {% endfor %}

    {% for meth in obj.methods() -%}
    {%- call ts::func_decl("public", meth, 4, "") %}
    {% endfor %}

    // UniffiObjectInterface
    destroy(): void {
        rustCall(callStatus => { NativeModule.{{ obj.ffi_object_free().name() }}(this.pointer, callStatus) });
    }

    uniffiClonePointer(): UnsafeMutableRawPointer {
        return rustCall(callStatus => NativeModule.{{ obj.ffi_object_clone().name() }}(this.pointer, callStatus) );
    }

    {%- for tm in obj.uniffi_traits() %}
    {%-     match tm %}
    {%-         when UniffiTrait::Display { fmt } %}
    open var description: String {
        return {% call ts::try(fmt) %} {{ fmt.return_type().unwrap()|lift_fn }}(
            {% call ts::to_ffi_call(fmt) %}
        )
    }
    {%-         when UniffiTrait::Debug { fmt } %}
    open var debugDescription: String {
        return {% call ts::try(fmt) %} {{ fmt.return_type().unwrap()|lift_fn }}(
            {% call ts::to_ffi_call(fmt) %}
        )
    }
    {%-         when UniffiTrait::Eq { eq, ne } %}
    public static func == (self: {{ impl_class_name }}, other: {{ impl_class_name }}) -> Bool {
        return {% call ts::try(eq) %} {{ eq.return_type().unwrap()|lift_fn }}(
            {% call ts::to_ffi_call(eq) %}
        )
    }
    {%-         when UniffiTrait::Hash { hash } %}
    open func hash(into hasher: inout Hasher) {
        let val = {% call ts::try(hash) %} {{ hash.return_type().unwrap()|lift_fn }}(
            {% call ts::to_ffi_call(hash) %}
        )
        hasher.combine(val)
    }
    {%-         else %}
    {%-    endmatch %}
    {%- endfor %}

}

function create{{ impl_class_name }}(pointer: UnsafeMutableRawPointer): {{ impl_class_name }} {
    const instance = Object.create({{ impl_class_name }}.prototype);
    instance.pointer = pointer;
    return instance;
}

{%- if obj.has_callback_interface() %}
{%- let callback_handler = format!("uniffiCallbackInterface{}", name) %}
{%- let callback_init = format!("uniffiCallbackInit{}", name) %}
{%- let vtable = obj.vtable().expect("trait interface should have a vtable") %}
{%- let vtable_methods = obj.vtable_methods() %}
{%- let ffi_init_callback = obj.ffi_init_callback() %}
{% include "CallbackInterfaceImpl.ts" %}
{%- endif %}

const {{ ffi_converter_name }} = (() => {
    const pointerConverter = FfiConverterUInt64;
    type TypeName = {{ type_name }};
    class FFIConverter implements FfiConverter<UnsafeMutableRawPointer, TypeName> {
        {%- if obj.has_callback_interface() %}
            {{- self.add_import_from("UniffiHandleMap", "handle-map")}}
        handleMap = UniffiHandleMap<{{ type_name }}>();
        lift(value: UnsafeMutableRawPointer): TypeName {
            // TODO look in a handle map.
            return create{{ type_name }}(value);
        }
        {% else %}
        lift(value: UnsafeMutableRawPointer): TypeName {
            return create{{ type_name }}(value);
        }
        {%- endif %}
        lower(value: TypeName): UnsafeMutableRawPointer {
            return value.uniffiClonePointer();
        }
        read(from: RustBuffer): TypeName {
            return this.lift(pointerConverter.read(from));
        }
        write(value: TypeName, into: RustBuffer): void {
            pointerConverter.write(this.lower(value), into);
        }
        allocationSize(value: TypeName): number {
            return pointerConverter.allocationSize(BigInt(0));
        }
    }
    return new FFIConverter();
})();
{#
    typealias FfiType = UnsafeMutableRawPointer
    typealias SwiftType = {{ type_name }}

    public static func lift(_ pointer: UnsafeMutableRawPointer) throws -> {{ type_name }} {
        return {{ impl_class_name }}(unsafeFromRawPointer: pointer)
    }

    public static func lower(_ value: {{ type_name }}) -> UnsafeMutableRawPointer {
        {%- if obj.has_callback_interface() %}
        guard let ptr = UnsafeMutableRawPointer(bitPattern: UInt(truncatingIfNeeded: handleMap.insert(obj: value))) else {
            fatalError("Cast to UnsafeMutableRawPointer failed")
        }
        return ptr
        {%- else %}
        return value.uniffiClonePointer()
        {%- endif %}
    }

    public static func read(from buf: inout (data: Data, offset: Data.Index)) throws -> {{ type_name }} {
        let v: UInt64 = try readInt(&buf)
        // The Rust code won't compile if a pointer won't fit in a UInt64.
        // We have to go via `UInt` because that's the thing that's the size of a pointer.
        let ptr = UnsafeMutableRawPointer(bitPattern: UInt(truncatingIfNeeded: v))
        if (ptr == nil) {
            throw UniffiInternalError.unexpectedNullPointer
        }
        return try lift(ptr!)
    }

    public static func write(_ value: {{ type_name }}, into buf: inout [UInt8]) {
        // This fiddling is because `Int` is the thing that's the same size as a pointer.
        // The Rust code won't compile if a pointer won't fit in a `UInt64`.
        writeInt(&buf, UInt64(bitPattern: Int64(Int(bitPattern: lower(value)))))
    }
}
#}

{# Objects as error #}
{%- if is_error %}
{# Due to some mismatches in the ffi converter mechanisms, errors are a RustBuffer holding a pointer #}
public struct {{ ffi_converter_name }}__as_error: FfiConverterArrayBuffer {
    public static func lift(_ buf: ArrayBuffer) throws -> {{ type_name }} {
        var reader = createReader(data: Data(rustBuffer: buf))
        return try {{ ffi_converter_name }}.read(from: &reader)
    }

    public static func lower(_ value: {{ type_name }}) -> RustBuffer {
        fatalError("not implemented")
    }

    public static func read(from buf: inout (data: Data, offset: Data.Index)) throws -> {{ type_name }} {
        fatalError("not implemented")
    }

    public static func write(_ value: {{ type_name }}, into buf: inout [UInt8]) {
        fatalError("not implemented")
    }
}
{%- endif %}

{#
We always write these public functions just in case the enum is used as
an external type by another crate.
#}
export function {{ ffi_converter_name }}_lift(pointer: UnsafeMutableRawPointer): {{ type_name }} {
    return {{ ffi_converter_name }}.lift(pointer)
}

export function {{ ffi_converter_name }}_lower(value: {{ type_name }}): UnsafeMutableRawPointer {
    return {{ ffi_converter_name }}.lower(value)
}
