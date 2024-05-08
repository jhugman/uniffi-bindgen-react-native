{%- let namespace = ci.namespace() %}
{%- let module_name = config.cpp_module.clone() %}
#pragma once
#include <jsi/jsi.h>
#include <iostream>
#include <map>

using namespace facebook;

class {{ module_name }} : public jsi::HostObject {
  protected:
    std::map<std::string,jsi::Value> props;
    {%- for func in ci.iter_ffi_function_definitions() %}
    static jsi::Value cpp_{{ func.name() }}(jsi::Runtime& rt, const jsi::Value& thisVal, const jsi::Value* args, size_t count);
    {%- endfor %}
  public:
    {{ module_name }}(jsi::Runtime &rt);

    static jsi::Object makeNativeObject(jsi::Runtime& rt);

    virtual ~{{ module_name }}();

    virtual jsi::Value get(jsi::Runtime& rt, const jsi::PropNameID& name);
    virtual void set(jsi::Runtime& rt,const jsi::PropNameID& name,const jsi::Value& value);
    virtual std::vector<jsi::PropNameID> getPropertyNames(jsi::Runtime& rt);
};
