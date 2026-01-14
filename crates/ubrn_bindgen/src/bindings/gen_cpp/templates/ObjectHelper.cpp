{%- import "macros.cpp" as cpp %}

{%- for obj in ci.object_definitions() %}
{%- let bless = obj.ffi_function_bless_handle() %}
{%- let free = obj.ffi_object_free() %}
{%- let uint64 = FfiType::UInt64 %}
{%- call cpp::cpp_fn_from_js_decl(bless) %} {
    auto handle = {{ uint64|bridging_class(ci) }}::fromJs(rt, callInvoker, args[0]);
    auto static destructor = [](uint64_t handle) {
        RustCallStatus status = {0};
        {{ free.name() }}(handle, &status);
    };
    auto ptrObj = std::make_shared<{{ ci.cpp_namespace_includes() }}::DestructibleObject>(handle, destructor);
    auto obj = jsi::Object::createFromHostObject(rt, ptrObj);
    return jsi::Value(rt, obj);
}
{%- endfor %}
