{%- let namespace = ci.namespace() %}
{%- let module_name = config.cpp_module.clone() %}
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

using namespace facebook;

// Initialization into the Hermes Runtime
extern "C" void registerNatives(jsi::Runtime &rt) {
    rt.global().setProperty(rt, "{{ module_name }}", {{ module_name }}::makeNativeObject(rt));
}

{% include "RustCallStatus.cpp" %}
{% include "Callback.cpp" %}
{% include "Handle.cpp" %}
{% include "RustArcPtrHelper.cpp" %}

// Calling into Rust.
extern "C" {
    {%- for func in ci.iter_ffi_function_definitions() %}
    {% call cpp::rust_fn_decl(func) %}
    {%- endfor %}
}

// This calls into Rust.
{% include "RustBufferHelper.cpp" %}

{{ module_name }}::{{ module_name }}(jsi::Runtime &rt) : props() {
    // Map from Javascript names to the cpp names
    {%- for func in ci.iter_ffi_functions_js_to_cpp() %}
    {%- let name = func.name() %}
    props["{{ name }}"] = jsi::Function::createFromHostFunction(
        rt,
        jsi::PropNameID::forAscii(rt, "{{ name }}"),
        2,
        {% call cpp::cpp_func_name(func) %}
    );
    {%- endfor %}
}

jsi::Object {{ module_name }}::makeNativeObject(jsi::Runtime& rt) {
    auto obj = std::make_shared<{{ module_name }}>(rt);
    auto rval = rt.global().createFromHostObject(rt,obj);

    return rval;
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
    // NOOP
}

{%- include "StringHelper.cpp" %}

// Methods calling directly into the uniffi generated C API of the Rust crate.
{%- for func in ci.iter_ffi_functions_js_to_rust() %}

{% call cpp::rust_fn_caller(module_name, func) %}
{%- endfor %}

{%- import "macros.cpp" as cpp %}
