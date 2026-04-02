{%- import "CallBodyMacros.ts" as cb %}
{%- import "ObjectInterfaceTemplate.ts" as oi %}
{%- import "CallbackInterfaceImpl.ts" as cbi_impl %}
{%- macro object(obj) %}

{#- Render the protocol: interface (default) or type alias (strict) -#}
{%- if !obj.strict_object_types || obj.has_callback_interface %}
{%- call oi::object_interface(obj) %}
{%- else %}
export type {{ obj.protocol_name }} = {{ obj.impl_class_name }};
{%- endif %}
{%- if !obj.strict_object_types && !obj.has_callback_interface %}
/**
 * @deprecated Use `{{ obj.protocol_name }}` instead.
 */
export type {{ obj.impl_class_name }}Interface = {{ obj.protocol_name }};
{%- endif %}

{% call cb::docstring(obj.docstring) %}
export class {{ obj.impl_class_name }} extends UniffiAbstractObject
{%- if !obj.strict_object_types || obj.has_callback_interface %} implements {{ obj.protocol_name }}{%- endif %} {

    readonly [uniffiTypeNameSymbol] = "{{ obj.impl_class_name }}";
    readonly [destructorGuardSymbol]: UniffiGcObject;
    readonly [pointerLiteralSymbol]: UniffiHandle;

    {%- match obj.primary_constructor %}
    {%- when Some with (cons) %}
    {%- if !cons.is_async() %}
    {%-   call _object_ctor_decl(obj, cons) %}
    {%- else %}
    {%- call _object_private_ctor(obj) %}

    // Async primary constructor declared for this class.
    {%-   call _object_method_decl(obj, "static", cons) %}
    {%- endif %}
    {%- when None %}
    // No primary constructor declared for this class.
    {%- call _object_private_ctor(obj) %}
    {%- endmatch %}

    {% for cons in obj.alternate_constructors %}
    {%- call _object_method_decl(obj, "static", cons) %}
    {% endfor %}

    {% for meth in obj.methods -%}
    {%- call _object_method_decl(obj, "", meth) %}
    {% endfor %}

    {%- for tm in obj.uniffi_traits %}
    {%      match tm %}
    {%-         when TsUniffiTrait::Display { method } %}
    toString(): string {
        {% call cb::call_body_method(method, obj.obj_factory) %}
    }
    {%-         when TsUniffiTrait::Debug { method } %}
    toDebugString(): string {
        {% call cb::call_body_method(method, obj.obj_factory) %}
    }
    {%-            if !obj.has_display_trait() %}
    toString(): string {
        return this.toDebugString();
    }
    {%            endif %}
    {%-         when TsUniffiTrait::Eq { eq, ne } %}
    equals(other: {{ obj.impl_class_name }}): {% call cb::return_type(eq) %} {
        {% call cb::call_body_method(eq, obj.obj_factory) %}
    }
    {%-         when TsUniffiTrait::Hash { method } %}
    hashCode(): {% call cb::return_type(method) %} {
        {% call cb::call_body_method(method, obj.obj_factory) %}
    }
    {%-         when TsUniffiTrait::Ord { cmp } %}
    compareTo(other: {{ obj.impl_class_name }}): {% call cb::return_type(cmp) %} {
        {% call cb::call_body_method(cmp, obj.obj_factory) %}
    }
    {%-    endmatch %}
    {%- endfor %}

    uniffiDestroy(): void {
        const ptr = (this as any)[destructorGuardSymbol];
        if (ptr !== undefined) {
            const pointer = {{ obj.obj_factory }}.pointer(this);
            {{ obj.obj_factory }}.freePointer(pointer);
            {{ obj.obj_factory }}.unbless(ptr);
            delete (this as any)[destructorGuardSymbol];
        }
    }

    static instanceOf(obj_: any): obj_ is {{ obj.impl_class_name }} {
        return {{ obj.obj_factory }}.isConcreteType(obj_);
    }

    {% if obj.is_error %}
    static hasInner(obj_: any): obj_ is UniffiThrownObject<{{ obj.impl_class_name }}> {
        return UniffiThrownObject.instanceOf(obj_) && {{ obj.impl_class_name }}.instanceOf(obj_.inner);
    }

    static getInner(err: UniffiThrownObject<{{ obj.impl_class_name }}>): {{ obj.impl_class_name }} {
        return err.inner;
    }
    {%- endif %}
}

const {{ obj.obj_factory }}: UniffiObjectFactory<{{ obj.protocol_name }}> = (() => {
    {% if obj.supports_finalization_registry %}
    /// <reference lib="es2021" />
    const registry = typeof FinalizationRegistry !== 'undefined' ? new FinalizationRegistry<UniffiHandle>((heldValue: UniffiHandle) => {
        {{ obj.obj_factory }}.freePointer(heldValue);
    }) : null;
    {% endif %}
    return {
    create(pointer: UniffiHandle): {{ obj.protocol_name }} {
        const instance = Object.create({{ obj.impl_class_name }}.prototype);
        instance[pointerLiteralSymbol] = pointer;
        instance[destructorGuardSymbol] = this.bless(pointer);
        instance[uniffiTypeNameSymbol] = "{{ obj.impl_class_name }}";
        return instance;
    },

    {% if obj.supports_finalization_registry %}
    bless(p: UniffiHandle): UniffiGcObject {
        const ptr = {
            p, // make sure this object doesn't get optimized away.
            markDestroyed: () => undefined,
        };
        if (registry) {
            registry.register(ptr, p, ptr);
        }
        return ptr;
    },

    unbless(ptr_: UniffiGcObject) {
        if (registry) {
            registry.unregister(ptr_);
        }
    },
    {%- else %}
    bless(p: UniffiHandle): UniffiGcObject {
        return uniffiCaller.rustCall(
            /*caller:*/ (status) =>
                nativeModule().{{ obj.ffi_bless_pointer }}(p, status),
            /*liftString:*/ FfiConverterString.lift
        );
    },

    unbless(ptr_: UniffiGcObject) {
        ptr_.markDestroyed();
    },
    {%- endif %}

    pointer(obj_: {{ obj.protocol_name }}): UniffiHandle {
        if ((obj_ as any)[destructorGuardSymbol] === undefined) {
            throw new UniffiInternalError.UnexpectedNullPointer();
        }
        return (obj_ as any)[pointerLiteralSymbol];
    },

    clonePointer(obj_: {{ obj.protocol_name }}): UniffiHandle {
        const pointer = this.pointer(obj_);
        return uniffiCaller.rustCall(
            /*caller:*/ (callStatus) => nativeModule().{{ obj.ffi_clone }}(pointer, callStatus),
            /*liftString:*/ FfiConverterString.lift
        );
    },

    freePointer(pointer: UniffiHandle): void {
        uniffiCaller.rustCall(
            /*caller:*/ (callStatus) => nativeModule().{{ obj.ffi_free }}(pointer, callStatus),
            /*liftString:*/ FfiConverterString.lift
        );
    },

    isConcreteType(obj_: any): obj_ is {{ obj.protocol_name }} {
        return obj_[destructorGuardSymbol] && obj_[uniffiTypeNameSymbol] === "{{ obj.impl_class_name }}";
    },
}})();

{%- if !obj.has_callback_interface %}
const {{ obj.ffi_converter_name }} = new FfiConverterObject({{ obj.obj_factory }});
{%- else %}
const {{ obj.ffi_converter_name }} = new FfiConverterObjectWithCallbacks({{ obj.obj_factory }});

// Add a vtable for the callbacks that go in {{ obj.ts_name }}.
{%- call cbi_impl::callback_interface_impl(obj.vtable.as_ref().expect("trait interface should have a vtable"), obj.ffi_converter_name, obj.trait_impl) %}
{%- endif %}

{%- if obj.is_error %}
const {{ obj.ffi_error_converter_name }} = new FfiConverterObjectAsError("{{ obj.decl_type_name }}", {{ obj.ffi_converter_name }});
{%- endif %}
{%- endmacro %}

{#- Macro: primary constructor declaration -#}
{%- macro _object_ctor_decl(obj, cons) %}
{%- call cb::docstring(cons.docstring) %}
    constructor(
    {%- call cb::arg_list_decl(cons) -%}) {%- call cb::throws_kw(cons) %} {
        super();
        const pointer =
            {% call cb::to_ffi_call(cons) %};
        this[pointerLiteralSymbol] = pointer;
        this[destructorGuardSymbol] = {{ obj.obj_factory }}.bless(pointer);
    }
{%- endmacro %}

{#- Macro: method or static method declaration -#}
{%- macro _object_method_decl(obj, func_decl, callable) %}
{%- call cb::docstring(callable.docstring) %}
    {% if !func_decl.is_empty() %}{{ func_decl }} {% endif %}{% if callable.is_async() %}async {% endif %}{{ callable.name }}(
    {%- call cb::arg_list_decl(callable) -%}): {# space #}
    {%- call cb::return_type(callable) %}
    {%- call cb::throws_kw(callable) %} {
    {%- if callable.receiver.is_some() %}
    {%- call cb::call_body_method(callable, obj.obj_factory) %}
    {%- else %}
    {%- call cb::call_body_function(callable) %}
    {%- endif %}
    }
{%- endmacro %}

{#- Macro: private constructor -#}
{%- macro _object_private_ctor(obj) %}
private constructor(pointer: UniffiHandle) {
    super();
    this[pointerLiteralSymbol] = pointer;
    this[destructorGuardSymbol] = {{ obj.obj_factory }}.bless(pointer);
}
{%- endmacro %}
