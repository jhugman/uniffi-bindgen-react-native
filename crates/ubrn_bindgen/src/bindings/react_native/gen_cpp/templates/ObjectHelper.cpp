{%- import "macros.cpp" as cpp %}

{%- for obj in ci.object_definitions() %}
{%- let bless = obj.ffi_function_bless_pointer() %}
{%- let free = obj.ffi_object_free() %}
{%- let uint64 = FfiType::UInt64 %}
{%- call cpp::cpp_fn_from_js_decl(bless) %} {
    auto pointer = {{ uint64|bridging_class(ci) }}::fromJs(rt, callInvoker, args[0]);
    auto static destructor = [](uint64_t p) {
        auto pointer = reinterpret_cast<void *>(static_cast<uintptr_t>(p));
        RustCallStatus status = {0};
        {{ free.name() }}(pointer, &status);
    };
    auto ptrObj = std::make_shared<{{ ci.cpp_namespace_includes() }}::DestructibleObject>(pointer, destructor);
    auto obj = jsi::Object::createFromHostObject(rt, ptrObj);
    return jsi::Value(rt, obj);
}
{%- endfor %}
