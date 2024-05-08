{%- let namespace = ci.namespace() %}
{%- let module_name = config.cpp_module.clone() %}
#include "{{ namespace }}.hpp"

#include <stdexcept>
#include <map>
#include <utility>
#include <iostream>

using namespace facebook;

extern "C" {
    {%- for func in ci.iter_ffi_function_definitions() %}
    int {{ func.name() }}(int a, int b);
    {%- endfor %}
}

{{ module_name }}::{{ module_name }}(jsi::Runtime &rt) : props() {
    {%- for func in ci.iter_ffi_function_definitions() %}
    {%- let name = func.name() %}
    props["{{ name }}"] = jsi::Function::createFromHostFunction(rt, jsi::PropNameID::forAscii(rt, "{{ name }}"), 2, cpp_{{ name }});
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
{%- for func in ci.iter_ffi_function_definitions() %}
{%- let func_name = func.name() %}
jsi::Value {{ module_name }}::cpp_{{ func_name }}(jsi::Runtime& rt, const jsi::Value& thisVal, const jsi::Value* args, size_t count) {
    auto a = args[0].getNumber();
    auto b = args[1].getNumber();
    auto sum = {{ func_name }}(a, b);
    return jsi::Value(sum);
}
{%- endfor %}

#include "registerNatives.h"
extern "C" void registerNatives(jsi::Runtime &rt) {
    rt.global().setProperty(rt, "{{ module_name }}", {{ module_name }}::makeNativeObject(rt));
}
