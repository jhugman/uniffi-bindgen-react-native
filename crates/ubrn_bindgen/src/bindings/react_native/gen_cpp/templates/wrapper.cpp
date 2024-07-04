{%- let namespace = ci.namespace() %}
{%- let module_name = config.cpp_module() %}
#ifndef UNIFFI_EXPORT
#if defined(_WIN32) || defined(_WIN64)
#define UNIFFI_EXPORT __declspec(dllexport)
#else
#define UNIFFI_EXPORT __attribute__((visibility("default")))
#endif
#endif

#include "{{ namespace }}.hpp"

#include "registerNatives.h"
#include "UniffiJsiTypes.h"
#include <stdexcept>
#include <map>
#include <utility>
#include <iostream>
#include <thread>

namespace react = facebook::react;
namespace jsi = facebook::jsi;

// Initialization into the Hermes Runtime
extern "C" void registerNatives(jsi::Runtime &rt, std::shared_ptr<react::CallInvoker> callInvoker) {
    {{ module_name }}::registerModule(rt, callInvoker);
}

{% include "RustCallStatus.cpp" %}
{% include "Callback.cpp" %}
{% include "Handle.cpp" %}

// Calling into Rust.
extern "C" {
    {%- for definition in ci.ffi_definitions() %}
    {%- match definition %}
    {%- when FfiDefinition::Struct(ffi_struct) %}
    {%- call cpp::callback_struct_decl(ffi_struct) %}
    {%- when FfiDefinition::CallbackFunction(callback) %}
    {%- call cpp::callback_fn_decl(callback) %}
    {%- when FfiDefinition::Function with(func) %}
    {% call cpp::rust_fn_decl(func) %}
    {%- else %}
    {%- endmatch %}
    {%- endfor %}
}

// This calls into Rust.
{% include "RustBufferHelper.cpp" %}

{%- for def in ci.ffi_definitions() %}
{%-   match def %}
{%-     when FfiDefinition::CallbackFunction(callback) %}
{%-       if callback.is_user_callback() %}
{%-         if callback.is_free_callback() %}
{%-           call cpp::callback_fn_free_impl(callback) %}
{%-         else %}
{%-           call cpp::callback_fn_impl(callback) %}
{%-         endif %}
{%-       endif %}
{%-     when FfiDefinition::Struct(ffi_struct) %}
{%-       if ffi_struct.is_vtable() %}
{%-         include "VTableStruct.cpp" %}
{%-       endif %}
{%-     else %}
{%-   endmatch %}
{%- endfor %}

{{ module_name }}::{{ module_name }}(
    jsi::Runtime &rt,
    std::shared_ptr<react::CallInvoker> invoker
) : props(), callInvoker(invoker) {
    // Map from Javascript names to the cpp names
    {%- for func in ci.iter_ffi_functions_js_to_cpp() %}
    {%- let name = func.name() %}
    props["{{ name }}"] = jsi::Function::createFromHostFunction(
        rt,
        jsi::PropNameID::forAscii(rt, "{{ name }}"),
        2,
        [this](jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args, size_t count) -> jsi::Value {
            return this->{% call cpp::cpp_func_name(func) %}(rt, thisVal, args, count);
        }
    );
    {%- endfor %}
}

void {{ module_name }}::registerModule(jsi::Runtime &rt, std::shared_ptr<react::CallInvoker> callInvoker) {
    auto tm = std::make_shared<{{ module_name }}>(rt, callInvoker);
    auto obj = rt.global().createFromHostObject(rt, tm);
    rt.global().setProperty(rt, "{{ module_name }}", obj);
}

void {{ module_name }}::unregisterModule(jsi::Runtime &rt) {
    // NOOP
}

jsi::Value {{ module_name }}::get(jsi::Runtime& rt, const jsi::PropNameID& name) {
    try {
        return jsi::Value(rt, props.at(name.utf8(rt)));
    }
    catch (std::out_of_range e) {
        return jsi::Value::undefined();
    }
}

std::vector<jsi::PropNameID> {{ module_name }}::getPropertyNames(jsi::Runtime& rt) {
    std::vector<jsi::PropNameID> rval;
    for (auto& [key, value] : props) {
        rval.push_back(jsi::PropNameID::forUtf8(rt, key));
    }
    return rval;
}

void {{ module_name }}::set(jsi::Runtime& rt, const jsi::PropNameID& name, const jsi::Value& value) {
    props.insert_or_assign(name.utf8(rt), &value);
}

{{ module_name }}::~{{ module_name }}() {
{%- for def in ci.ffi_definitions() %}
{%-   match def %}
{%-     when FfiDefinition::CallbackFunction(callback) %}
{%-       if callback.is_user_callback() %}
{%-         if callback.is_free_callback() %}
{%-           call cpp::callback_fn_free_cleanup(callback) %}
{%-         else %}
{%-           call cpp::callback_fn_cleanup(callback) %}
{%-         endif %}
{%-       endif %}
{%-     else %}
{%-   endmatch %}
{%- endfor %}
}

{%- include "StringHelper.cpp" %}

// Methods calling directly into the uniffi generated C API of the Rust crate.
{%- for func in ci.iter_ffi_functions_js_to_rust() %}
{% call cpp::rust_fn_caller(module_name, func) %}
{%- endfor %}

{%- for func in ci.iter_ffi_functions_init_callback() %}
{% call cpp::callback_init(module_name, func) %}
{%- endfor %}

{%- import "macros.cpp" as cpp %}
