{%- if self.include_once_check("ObjectRuntime.ts") %}
{%- include "ObjectRuntime.ts" %}
{%- endif %}
{%- let obj = ci|get_object_definition(name) %}
{%- let (protocol_name, impl_class_name) = obj|object_names(ci) %}
{%- let obj_factory = format!("uniffiType{}ObjectFactory", type_name) %}
{%- let methods = obj.methods() %}

{%- let is_error = ci.is_name_used_as_error(name) %}

{%- include "ObjectInterfaceTemplate.ts" %}

{% call ts::docstring(obj, 0) %}
export class {{ impl_class_name }} extends AbstractUniffiObject implements {{ protocol_name }}
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

    private _rustArcPtr: UniffiRustArcPtr;

    {%- match obj.primary_constructor() %}
    {%- when Some with (cons) %}
    {%- call ts::ctor_decl(obj_factory, cons, 4) %}
    {%- when None %}
    // No primary constructor declared for this class.
    private constructor(pointer: UnsafeMutableRawPointer) {
        super();
        this._rustArcPtr = {{ obj_factory }}.bless(pointer);
    }
    {%- endmatch %}

    {% for cons in obj.alternate_constructors() %}
    {%- call ts::method_decl("public static", obj_factory, cons, 4) %}
    {% endfor %}

    {% for meth in obj.methods() -%}
    {%- call ts::method_decl("public", obj_factory, meth, 4) %}
    {% endfor %}

    // AbstractUniffiObject
    uniffiDestroy(): void {
        if ((this as any)._rustArcPtr) {
            const pointer = {{ obj_factory }}.pointer(this);
            this._rustArcPtr.d(pointer);
            delete (this as any)._rustArcPtr;
        }
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

const {{ obj_factory }}: UniffiObjectFactory<{{type_name}}> = {
    create(pointer: UnsafeMutableRawPointer): {{ type_name }} {
        const instance = Object.create({{ type_name }}.prototype);
        instance._rustArcPtr = this.bless(pointer);
        return instance;
    },

    bless(pointer: UnsafeMutableRawPointer): UniffiRustArcPtr {
        const destructor = this.freePointer;
        return uniffiBlessPointer(pointer, destructor);
    },

    pointer(obj: {{ type_name }}): UnsafeMutableRawPointer {
        const ptr = (obj as any)._rustArcPtr;
        if (ptr === undefined) {
            throw new UniffiInternalError.UnexpectedNullPointer();
        }
        return ptr.p;
    },

    clonePointer(obj: {{ type_name }}): UnsafeMutableRawPointer {
        const pointer = this.pointer(obj);
        return rustCall(callStatus => NativeModule.{{ obj.ffi_object_clone().name() }}(pointer, callStatus) );
    },

    freePointer(pointer: UnsafeMutableRawPointer): void {
        rustCall(callStatus => { NativeModule.{{ obj.ffi_object_free().name() }}(pointer, callStatus) });
    }
};

{%- if obj.has_callback_interface() %}
{%- let callback_handler = format!("uniffiCallbackInterface{}", name) %}
{%- let callback_init = format!("uniffiCallbackInit{}", name) %}
{%- let vtable = obj.vtable().expect("trait interface should have a vtable") %}
{%- let vtable_methods = obj.vtable_methods() %}
{%- let ffi_init_callback = obj.ffi_init_callback() %}
{% include "CallbackInterfaceImpl.ts" %}
{%- endif %}

// FfiConverter for {{ type_name }}
const {{ ffi_converter_name }} =  new FfiConverterObject({{ obj_factory }});
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
public struct {{ ffi_converter_name }}__as_error: AbstractFfiConverterArrayBuffer {
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

{{- self.export_converter(ffi_converter_name) -}}
