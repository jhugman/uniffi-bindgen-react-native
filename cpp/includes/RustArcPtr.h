/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
namespace uniffi_jsi {
using namespace facebook;
using CallInvoker = uniffi_runtime::UniffiCallInvoker;

template <> struct Bridging<void *> {
  static void *fromJs(jsi::Runtime &rt,
                      std::shared_ptr<CallInvoker> callInvoker,
                      const jsi::Value &value) {
    try {
      auto num = uniffi_jsi::Bridging<uint64_t>::fromJs(rt, callInvoker, value);
      return reinterpret_cast<void *>(static_cast<uintptr_t>(num));
    } catch (const std::logic_error &e) {
      throw jsi::JSError(rt, e.what());
    }
  }

  static jsi::Value toJs(jsi::Runtime &rt,
                         std::shared_ptr<CallInvoker> callInvoker,
                         void *value) {
    auto num = static_cast<uint64_t>(reinterpret_cast<uintptr_t>(value));
    return uniffi_jsi::Bridging<uint64_t>::toJs(rt, callInvoker, num);
  }
};

/// Constructs a `DestructibleObject` that accepts a pointer and destructor
/// callback. This corresponds to a `UniffiRustArcPtr` object. The idea is that
/// this object has a C++ destructor which calls the Rust drop function
/// for the object.
///
/// The JS object provides a `markDestroyed` function. This will override the
/// behaviour of the C++ destructor.
///
/// This allows the generated JS to call the Rust object's `free` function
/// in the usual manner, including checks for panics, which are translated
/// into and thrown as JS errors.
///
/// It should be noted that this is a hack: the C++ destructor. The relevant
/// comment on the HostObject's destructor is reproduced here:
///
///   // The C++ object's dtor will be called when the GC finalizes this
///   // object.  (This may be as late as when the Runtime is shut down.)
///   // You have no control over which thread it is called on.  This will
///   // be called from inside the GC…
///
/// Until hermes and React Native gain a `FinalizationRegistry`, this is
/// unlikely to get better.
class DestructibleObject : public jsi::HostObject {
private:
  std::mutex destructorMutex;
  bool isDestroyed = false;
  uint64_t pointer;
  std::function<void(uint64_t)> destructor;

public:
  DestructibleObject(uint64_t pointer, std::function<void(uint64_t)> destructor)
      : jsi::HostObject(), pointer(pointer), destructor(destructor) {}

  ~DestructibleObject() {
    /// You can use uniffiDestroy() from JS instead of relying on GC.
    ///
    /// From
    /// https://github.com/facebook/hermes/issues/982#issuecomment-1771325667:
    ///
    /// "Generally speaking, when dealing with native resources, it is
    /// recommended to free the native resource explicitly when you are done
    /// with it, because garbage collection is unpredictable and should not be
    /// relied on for freeing native resources timely."
    destroy_object();
  }

  void destroy_object() {
    std::lock_guard<std::mutex> lock(destructorMutex);
    if (!isDestroyed) {
      isDestroyed = true;
      destructor(pointer);
    }
  }

  jsi::Value get(jsi::Runtime &rt, const jsi::PropNameID &name) override {
    std::string propName = name.utf8(rt);
    if (propName == "markDestroyed") {
      auto func = jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forAscii(rt, "markDestroyed"), 0,
          [this](jsi::Runtime &rt, const jsi::Value &thisVal,
                 const jsi::Value *args, size_t count) -> jsi::Value {
            std::lock_guard<std::mutex> lock(this->destructorMutex);
            this->isDestroyed = true;
            return jsi::Value::undefined();
          });
      return jsi::Value(rt, func);
    }
    return jsi::Value::undefined();
  }
};

} // namespace uniffi_jsi
