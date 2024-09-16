{{- self.import_infra("UniffiAbstractObject", "objects") -}}
{{- self.import_infra_type("UnsafeMutableRawPointer", "objects") -}}
{{- self.import_infra("FfiConverterObject", "objects") -}}
{{- self.import_infra_type("UniffiObjectFactory", "objects") -}}
{{- self.import_infra_type("FfiConverter", "ffi-converters") -}}
{{- self.import_infra_type("UniffiRustArcPtr", "rust-call") }}
{{- self.import_infra("destructorGuardSymbol", "symbols") -}}
{{- self.import_infra("pointerLiteralSymbol", "symbols") -}}
{{- self.import_infra("uniffiTypeNameSymbol", "symbols") -}}

{%- let obj = ci|get_object_definition(name) %}
{%- let protocol_name = obj|type_name(self) %}
{%- let impl_class_name = obj|decl_type_name(self) %}
{%- let obj_factory = format!("uniffiType{}ObjectFactory", impl_class_name) %}
{%- let methods = obj.methods() %}

{%- let is_error = ci.is_name_used_as_error(name) %}

{%- include "ObjectInterfaceTemplate.ts" %}
{%- macro private_ctor() %}
private constructor(pointer: UnsafeMutableRawPointer) {
    super();
    this[pointerLiteralSymbol] = pointer;
    this[destructorGuardSymbol] = {{ obj_factory }}.bless(pointer);
}
{%- endmacro %}

{% call ts::docstring(obj, 0) %}
export class {{ impl_class_name }} extends UniffiAbstractObject implements {{ protocol_name }} {

    readonly [uniffiTypeNameSymbol] = "{{ impl_class_name }}";
    readonly [destructorGuardSymbol]: UniffiRustArcPtr;
    readonly [pointerLiteralSymbol]: UnsafeMutableRawPointer;

    {%- match obj.primary_constructor() %}
    {%- when Some with (cons) %}
    {%- if !cons.is_async() %}
    {%-   call ts::ctor_decl(obj_factory, cons, 4) %}
    {%- else %}
    {%- call private_ctor() %}

    // Async primary constructor declared for this class.
    {%-   call ts::method_decl("public static", obj_factory, cons, 4) %}
    {%- endif %}
    {%- when None %}
    // No primary constructor declared for this class.
    {%- call private_ctor() %}
    {%- endmatch %}

    {% for cons in obj.alternate_constructors() %}
    {%- call ts::method_decl("public static", obj_factory, cons, 4) %}
    {% endfor %}

    {% for meth in obj.methods() -%}
    {%- call ts::method_decl("public", obj_factory, meth, 4) %}
    {% endfor %}

    {%- for tm in obj.uniffi_traits() %}
    {%      match tm %}
    {%-         when UniffiTrait::Display { fmt } %}
    /**
     * Calls into the `{{ type_name }}::to_string()` method of the native Rust peer.
     *
     * Generated by deriving the `Display` trait in Rust.
     */
    toString(): string {
        {% call ts::call_body(obj_factory, fmt) %}
    }
    {%-         when UniffiTrait::Debug { fmt } %}
    /**
     * Calls into the `Debug` string representation of `{{ type_name }}` (the native Rust peer).
     *
     * Generated by deriving the `Debug` trait in Rust.
     */
    toDebugString(): string {
        {% call ts::call_body(obj_factory, fmt) %}
    }
    {%-            if !obj.has_uniffi_trait("Display") %}

    /**
     * Calls into the `Debug` string representation of `{{ type_name }}` (the native Rust peer).
     *
     * Generated by deriving the `Debug` trait in Rust, without deriving `Display`.
     */
    toString(): string {
        return this.toDebugString();
    }
    {%            endif %}
    {%-         when UniffiTrait::Eq { eq, ne } %}
    /**
     * Calls into the `==` method of `{{ type_name }}` (the native Rust peer).
     *
     * Returns `true` if and only if the two instance of `{{ type_name }}` are
     * equivalent on the Rust side.
     *
     * Generated by deriving the `Eq` trait in Rust.
     */
    equals(other: {{ impl_class_name }}): {% call ts::return_type(eq) %} {
        {% call ts::call_body(obj_factory, eq) %}
    }
    {%-         when UniffiTrait::Hash { hash } %}
    /**
     * Calls into the `hash` method of `{{ type_name }}` (the native Rust peer).
     *
     * Generated by deriving the `Hash` trait in Rust.
     */
    hashCode(): {% call ts::return_type(hash) %} {
        {% call ts::call_body(obj_factory, hash) %}
    }
    {%-         else %}
    {%-    endmatch %}
    {%- endfor %}

    /**
     * {@inheritDoc uniffi-bindgen-react-native#UniffiAbstractObject.uniffiDestroy}
     */
    uniffiDestroy(): void {
        if ((this as any)[destructorGuardSymbol]) {
            const pointer = {{ obj_factory }}.pointer(this);
            {{ obj_factory }}.freePointer(pointer);
            this[destructorGuardSymbol].markDestroyed();
            delete (this as any)[destructorGuardSymbol];
        }
    }

    static instanceOf(obj: any): obj is {{ impl_class_name }} {
        return {{ obj_factory }}.isConcreteType(obj);
    }

    {% if is_error %}
    {{- self.import_infra("UniffiThrownObject", "objects") }}
    static hasInner(obj: any): obj is UniffiThrownObject<{{ impl_class_name }}> {
        return UniffiThrownObject.instanceOf(obj) && {{ impl_class_name }}.instanceOf(obj.inner);
    }

    static getInner(err: UniffiThrownObject<{{ impl_class_name }}>): {{ impl_class_name }} {
        return err.inner;
    }
    {%- endif %}
}

const {{ obj_factory }}: UniffiObjectFactory<{{ type_name }}> = {
    create(pointer: UnsafeMutableRawPointer): {{ type_name }} {
        const instance = Object.create({{ impl_class_name }}.prototype);
        instance[pointerLiteralSymbol] = pointer;
        instance[destructorGuardSymbol] = this.bless(pointer);
        instance[uniffiTypeNameSymbol] = "{{ impl_class_name }}";
        return instance;
    },

    bless(p: UnsafeMutableRawPointer): UniffiRustArcPtr {
        return rustCall(
            /*caller:*/ (status) =>
                nativeModule().{{ obj.ffi_function_bless_pointer().name() }}(p, status),
            /*liftString:*/ FfiConverterString.lift
        );
    },

    pointer(obj: {{ type_name }}): UnsafeMutableRawPointer {
        if ((obj as any)[destructorGuardSymbol] === undefined) {
            throw new UniffiInternalError.UnexpectedNullPointer();
        }
        return (obj as any)[pointerLiteralSymbol];
    },

    clonePointer(obj: {{ type_name }}): UnsafeMutableRawPointer {
        const pointer = this.pointer(obj);
        return rustCall(
            /*caller:*/ (callStatus) => nativeModule().{{ obj.ffi_object_clone().name() }}(pointer, callStatus),
            /*liftString:*/ FfiConverterString.lift
        );
    },

    freePointer(pointer: UnsafeMutableRawPointer): void {
        rustCall(
            /*caller:*/ (callStatus) => nativeModule().{{ obj.ffi_object_free().name() }}(pointer, callStatus),
            /*liftString:*/ FfiConverterString.lift
        );
    },

    isConcreteType(obj: any): obj is {{ type_name }} {
        return obj[destructorGuardSymbol] && obj[uniffiTypeNameSymbol] === "{{ impl_class_name }}";
    },
};

{%- if !obj.has_callback_interface() %}
// FfiConverter for {{ type_name }}
const {{ ffi_converter_name }} =  new FfiConverterObject({{ obj_factory }});
{%- else %}
{{- self.import_infra("FfiConverterObjectWithCallbacks", "objects") }}
// FfiConverter for {{ type_name }}
const {{ ffi_converter_name }} = new FfiConverterObjectWithCallbacks({{ obj_factory }});

// Add a vtavble for the callbacks that go in {{ type_name }}.
{%- let vtable = obj.vtable().expect("trait interface should have a vtable") %}
{%- let cbi = obj %}
{% include "CallbackInterfaceImpl.ts" %}
{%- endif %}

{#- Objects as error #}
{%- if is_error %}
{%- let ffi_error_converter_name = type_|ffi_error_converter_name(self) %}
{{- self.import_infra("FfiConverterObjectAsError", "objects") }}
// FfiConverter for {{ type_name }} as an error.
const {{ ffi_error_converter_name }} = new FfiConverterObjectAsError("{{ decl_type_name }}", {{ ffi_converter_name }});
{{- self.export_converter(ffi_error_converter_name) -}}
{%- endif %}

{{- self.export_converter(ffi_converter_name) -}}
