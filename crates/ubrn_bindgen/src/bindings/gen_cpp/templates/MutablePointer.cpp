{%- let ns = ci.cpp_namespace() %}
namespace {{ ns }} {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

template <typename T>
class MutablePointerHostObject : public jsi::HostObject {
private:
  T* ptr_;
  std::shared_ptr<CallInvoker> invoker_;

public:
  MutablePointerHostObject(std::shared_ptr<CallInvoker> invoker, T* ptr) : ptr_(ptr), invoker_(std::move(invoker)) {}

  jsi::Value get(jsi::Runtime& rt, const jsi::PropNameID& prop) override {
    auto name = prop.utf8(rt);
    if (name == "set_value") {
      return jsi::Function::createFromHostFunction(
          rt, prop, 1,
          [this](jsi::Runtime& rt, const jsi::Value& thisVal, const jsi::Value* args, size_t count) -> jsi::Value {
            if (count < 1) {
              throw jsi::JSError(rt, "set_value requires one argument");
            }
            *ptr_ = Bridging<T>::fromJs(rt, invoker_, args[0]);
            return jsi::Value::undefined();
          });
    }
    return jsi::HostObject::get(rt, prop);
  }

  void set(jsi::Runtime& rt, const jsi::PropNameID& prop, const jsi::Value& val) override {
    // No direct property setters; mutations happen via set_value method.
    jsi::HostObject::set(rt, prop, val);
  }
};

template <typename T>
struct Bridging<T*> {
  static jsi::Value toJs(jsi::Runtime &rt,
                         std::shared_ptr<CallInvoker> invoker,
                         T* value) {
    auto host = std::make_shared<MutablePointerHostObject<T>>(invoker, value);
    return jsi::Object::createFromHostObject(rt, std::move(host));
  }
};

} // namespace {{ ns }}
