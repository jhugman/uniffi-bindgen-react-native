/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
namespace uniffi_jsi {
using namespace facebook;

/// Constructs a `DestructibleObject` that accepts a pointer and JS destructor
/// callback. This corresponds to a `UniffiRustArcPtr` object. The idea is that
/// this object has a C++ destructor which calls back into a JS "destructor" to
/// call the Rust drop function for the object. This re-entrancy is unstable, as
/// it relies on a C++ destruction being the same as JS destruction, and during
/// Hermes end-of-program shutdown, casues segfault as the NativeModule is
/// unloaded before the DestructibleObject goes out of memory.
class DestructibleObject : public jsi::HostObject {
private:
  jsi::Runtime &rt;
  uint64_t pointer;
  std::shared_ptr<jsi::Function> destructor;

public:
  DestructibleObject(jsi::Runtime &rt, uint64_t pointer,
                     std::shared_ptr<jsi::Function> destructor)
      : jsi::HostObject(), rt(rt), pointer(pointer), destructor(destructor) {}

  ~DestructibleObject() {
    /// Disabling the destructor behaviour here.
    /// Use uniffiDestroy() from JS instead of relying on GC.
    ///
    /// From
    /// https://github.com/facebook/hermes/issues/982#issuecomment-1771325667:
    ///
    /// "Generally speaking, when dealing with native resources, it is
    /// recommended to free the native resource explicitly when you are done
    /// with it, because garbage collection is unpredictable and should not be
    /// relied on for freeing native resources timely."
    // destroy_object();
  }

  void destroy_object() {
    try {
      auto ptr = uniffi_jsi::Bridging<uint64_t>::toJs(rt, pointer);
      if (destructor->isFunction(rt)) {
        destructor->call(rt, ptr);
      }
    } catch (const std::exception &e) {
      std::cout << "Caught exception: " << std::endl;
    } catch (...) {
      std::cout << "Error in destructor" << std::endl;
    }
  }

  jsi::Value get(jsi::Runtime &rt, const jsi::PropNameID &name) override {
    std::string propName = name.utf8(rt);
    if (propName == "p") {
      return uniffi_jsi::Bridging<uint64_t>::toJs(rt, pointer);
    } else if (propName == "d") {
      return jsi::Value(rt, *destructor);
    }
    return jsi::Value::undefined();
  }
};

template <> struct Bridging<void *> {
  static jsi::Value bless_pointer(jsi::Runtime &rt, const jsi::Value &pointer_v,
                                  const jsi::Value &destructor_v) {
    auto ptr = uniffi_jsi::Bridging<uint64_t>::fromJs(rt, pointer_v);
    auto destructor = std::make_shared<jsi::Function>(
        destructor_v.getObject(rt).getFunction(rt));
    auto ptrObj = std::make_shared<DestructibleObject>(rt, ptr, destructor);
    auto obj = jsi::Object::createFromHostObject(rt, ptrObj);
    return jsi::Value(rt, obj);
  }

  static void *fromJs(jsi::Runtime &rt, const jsi::Value &value) {
    try {
      auto num = uniffi_jsi::Bridging<uint64_t>::fromJs(rt, value);
      return reinterpret_cast<void *>(static_cast<uintptr_t>(num));
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt, void *value) {
    auto num = static_cast<uint64_t>(reinterpret_cast<uintptr_t>(value));
    return uniffi_jsi::Bridging<uint64_t>::toJs(rt, num);
  }
};
} // namespace uniffi_jsi
