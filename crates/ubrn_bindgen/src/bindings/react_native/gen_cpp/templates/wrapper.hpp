{%- let namespace = ci.namespace() %}
{%- let module_name = config.cpp_module() %}
#pragma once
#include <jsi/jsi.h>
#include <iostream>
#include <map>

using namespace facebook;

class {{ module_name }} : public jsi::HostObject {
  protected:
    std::map<std::string,jsi::Value> props;
    {%- for func in ci.iter_ffi_functions_js_to_cpp() %}
    static jsi::Value {% call cpp::cpp_func_name(func) %}(jsi::Runtime& rt, const jsi::Value& thisVal, const jsi::Value* args, size_t count);
    {%- endfor %}
  public:
    {{ module_name }}(jsi::Runtime &rt);

    /**
     * The entry point into the crate.
     *
     * React Native must call `{{ module_name }}.registerModule(rt)` before using
     * the Javascript interface.
     */
    static void registerModule(jsi::Runtime &rt);

    /**
     * Some cleanup into the crate goes here.
     *
     * Current implementation is empty, however, this is not guaranteed to always be the case.
     *
     * Clients should call `{{ module_name }}.unregisterModule(rt)` after final use where possible.
     */
    static void unregisterModule(jsi::Runtime &rt);

    static jsi::Object makeNativeObject(jsi::Runtime &rt);

    virtual ~{{ module_name }}();

    virtual jsi::Value get(jsi::Runtime& rt, const jsi::PropNameID& name);
    virtual void set(jsi::Runtime& rt,const jsi::PropNameID& name,const jsi::Value& value);
    virtual std::vector<jsi::PropNameID> getPropertyNames(jsi::Runtime& rt);
};

{%- import "macros.cpp" as cpp %}
