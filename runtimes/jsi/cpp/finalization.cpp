/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#include "finalization.h"

#include <atomic>
#include <cstdint>
#include <mutex>
#include <string>
#include <unordered_map>
#include <utility>
#include <vector>

namespace jsi = facebook::jsi;

namespace ubrn_fin {
namespace {

class Registry;

// Names a registry for as long as anything might refer to it. A Finalizer can
// outlive the registries it lists, and a later registry could reuse a freed
// one's address.
std::atomic<uint64_t> g_nextRegistrySerial{1};

[[noreturn]] void throwTypeError(jsi::Runtime &rt, const std::string &message) {
  auto ctor = rt.global().getPropertyAsFunction(rt, "TypeError");
  throw jsi::JSError(
      rt,
      ctor.callAsConstructor(
          rt, jsi::String::createFromUtf8(
                  rt, "uniffi jsi player: FinalizationRegistry: " + message)));
}

// A registry's queue of registrations whose targets have been collected.
//
// The only part of a registry a target's destructor reaches, so it holds no
// JSI value: those destructors run inside the collector, and as late as the
// runtime's own destruction.
class Collected {
public:
  explicit Collected(std::shared_ptr<facebook::react::CallInvoker> invoker)
      : invoker_(std::move(invoker)) {}

  // Set once, before JS can register anything. Weak, so that a posted drain
  // keeps nothing of the registry alive: it finds this expired once the
  // registry has gone, with its runtime or before it.
  std::weak_ptr<Registry> registry;

  // From a target's destructor: on any thread, possibly mid-collection.
  // Touches this queue's mutex and the host's invoker, never the runtime.
  void push(uint64_t id) noexcept;

  // On the JS thread. The next push after this posts a drain of its own, so a
  // target collected while the cleanups run is not stranded.
  std::vector<uint64_t> take() {
    std::lock_guard<std::mutex> lk(mtx_);
    drainPosted_ = false;
    return std::exchange(ids_, {});
  }

  // The registry is gone, so nothing could run a cleanup for what is queued.
  void close() {
    std::lock_guard<std::mutex> lk(mtx_);
    closed_ = true;
    ids_.clear();
  }

private:
  std::shared_ptr<facebook::react::CallInvoker> invoker_;
  std::mutex mtx_;
  std::vector<uint64_t> ids_;
  bool drainPosted_ = false;
  bool closed_ = false;
};

// The native state on every object registered as a target or passed as an
// unregister token. Generated bindings pass the same object as both.
//
// Its lists change only on the JS thread, while JS can still reach the object.
// The destructor reads them only once it cannot.
//
// A token keeps an entry per registration until it is collected or passed to
// `unregister`, even after that registration's target has been cleaned up: the
// drain cannot reach the token to prune it.
class Finalizer : public jsi::NativeState {
public:
  struct AsTarget {
    std::shared_ptr<Collected> collected;
    uint64_t id;
  };
  struct AsToken {
    uint64_t registry;
    uint64_t id;
  };

  std::vector<AsTarget> targetOf;
  std::vector<AsToken> tokenOf;

  ~Finalizer() override {
    for (const auto &t : targetOf) {
      t.collected->push(t.id);
    }
  }
};

// The Finalizer on `obj`, or null if it has no native state at all. Throws for
// one that carries another native state, which a destructor cannot share.
std::shared_ptr<Finalizer>
existingFinalizer(jsi::Runtime &rt, const jsi::Object &obj, const char *role) {
  if (!obj.hasNativeState(rt)) {
    return nullptr;
  }
  auto existing = std::dynamic_pointer_cast<Finalizer>(obj.getNativeState(rt));
  if (existing == nullptr) {
    throwTypeError(rt, std::string("the ") + role +
                           " already carries another native state");
  }
  return existing;
}

std::shared_ptr<Finalizer> attachFinalizer(jsi::Runtime &rt,
                                           const jsi::Object &obj) {
  auto created = std::make_shared<Finalizer>();
  obj.setNativeState(rt, created);
  return created;
}

// What only the JS thread touches: the cleanup and each live registration's
// held value.
//
// Owned by the registry object's host functions, so it dies with that object,
// at the latest when the runtime is destroyed, and never after it. The JSI
// values it holds are GC roots: a cleanup that refers back to its registry, as
// a generated object factory's does, keeps both alive until then.
class Registry {
public:
  Registry(jsi::Function cleanup, std::shared_ptr<Collected> collected)
      : cleanup_(std::move(cleanup)), collected_(std::move(collected)) {}

  ~Registry() { collected_->close(); }

  void registerTarget(jsi::Runtime &rt, const jsi::Value &target,
                      const jsi::Value &heldValue, const jsi::Value &token) {
    if (!target.isObject()) {
      throwTypeError(rt, "register: the target must be an object");
    }
    // A held value is held strongly, so one that is its own target would
    // never be collected.
    if (jsi::Value::strictEquals(rt, target, heldValue)) {
      throwTypeError(rt, "register: the target and held value are the same");
    }
    if (!token.isUndefined() && !token.isObject()) {
      throwTypeError(rt, "register: the unregister token must be an object");
    }
    auto targetObj = target.getObject(rt);
    auto asTarget = existingFinalizer(rt, targetObj, "target");
    std::shared_ptr<Finalizer> asToken;
    if (token.isObject()) {
      asToken = existingFinalizer(rt, token.getObject(rt), "unregister token");
    }
    // Attached only once both have passed, so that a token carrying another
    // native state leaves the target as it was.
    if (asTarget == nullptr) {
      asTarget = attachFinalizer(rt, targetObj);
    }
    if (token.isObject() && asToken == nullptr) {
      // Generated bindings pass the target as its own token.
      asToken = jsi::Value::strictEquals(rt, token, target)
                    ? asTarget
                    : attachFinalizer(rt, token.getObject(rt));
    }

    const uint64_t id = nextId_++;
    held_.emplace(id, jsi::Value(rt, heldValue));
    asTarget->targetOf.push_back({collected_, id});
    if (asToken != nullptr) {
      asToken->tokenOf.push_back({serial_, id});
    }
  }

  bool unregister(jsi::Runtime &rt, const jsi::Value &token) {
    if (!token.isObject()) {
      throwTypeError(rt, "unregister: the token must be an object");
    }
    auto obj = token.getObject(rt);
    if (!obj.hasNativeState(rt)) {
      return false;
    }
    auto f = std::dynamic_pointer_cast<Finalizer>(obj.getNativeState(rt));
    if (f == nullptr) {
      return false;
    }
    bool removed = false;
    auto &tokenOf = f->tokenOf;
    for (auto it = tokenOf.begin(); it != tokenOf.end();) {
      if (it->registry != serial_) {
        ++it;
        continue;
      }
      // A registration whose target was collected and drained since has no
      // held value left, and was not this call's to remove.
      removed |= held_.erase(it->id) > 0;
      // The token is its own target here, as in generated bindings, so its
      // destructor need not queue what can no longer be cleaned up.
      auto &targetOf = f->targetOf;
      for (auto t = targetOf.begin(); t != targetOf.end(); ++t) {
        if (t->id == it->id && t->collected == collected_) {
          targetOf.erase(t);
          break;
        }
      }
      it = tokenOf.erase(it);
    }
    return removed;
  }

  // On the JS thread, from a task the invoker runs between other JS work.
  void drain(jsi::Runtime &rt) {
    for (uint64_t id : collected_->take()) {
      auto it = held_.find(id);
      // Unregistered after its target was collected.
      if (it == held_.end()) {
        continue;
      }
      jsi::Value heldValue = std::move(it->second);
      held_.erase(it);
      // Swallowed, so a cleanup that throws does not cost the rest theirs.
      // There is nobody to report it to: the generated cleanup only frees, and
      // a free that fails here would have leaked without a registry anyway.
      try {
        cleanup_.call(rt, heldValue);
      } catch (...) {
      }
    }
  }

private:
  const uint64_t serial_ =
      g_nextRegistrySerial.fetch_add(1, std::memory_order_relaxed);
  uint64_t nextId_ = 1;
  jsi::Function cleanup_;
  std::unordered_map<uint64_t, jsi::Value> held_;
  std::shared_ptr<Collected> collected_;
};

void Collected::push(uint64_t id) noexcept {
  // Whether this call claimed the drain, so that a failure to post it can give
  // the claim back.
  bool post = false;
  try {
    {
      std::lock_guard<std::mutex> lk(mtx_);
      // Without an invoker nothing could ever drain the queue.
      if (closed_ || invoker_ == nullptr) {
        return;
      }
      ids_.push_back(id);
      post = !drainPosted_;
      drainPosted_ = true;
    }
    // Outside the lock, as cb_dispatch posts: never call into the host's
    // scheduler underneath a mutex of our own.
    if (post) {
      invoker_->invokeAsync([weak = registry](jsi::Runtime &rt) {
        if (auto r = weak.lock()) {
          r->drain(rt);
        }
      });
    }
  } catch (...) {
    // A destructor that throws terminates, so nothing may escape.
    //
    // An id that was never queued leaks its one object, as it would have
    // without a registry. One queued without its drain is worse: every later
    // push would see a drain pending, post none, and the registry would free
    // nothing more. So the claim is given back, and the next push posts a
    // drain that takes everything queued, this id included; until then, those
    // objects wait. A lock that fails here leaves the claim stuck.
    if (post) {
      try {
        std::lock_guard<std::mutex> lk(mtx_);
        drainPosted_ = false;
      } catch (...) {
      }
    }
  }
}

} // namespace

jsi::Value createFinalizationRegistry(
    jsi::Runtime &rt, std::shared_ptr<facebook::react::CallInvoker> invoker,
    const jsi::Value &cleanup) {
  if (!cleanup.isObject() || !cleanup.getObject(rt).isFunction(rt)) {
    throwTypeError(rt, "the cleanup must be a function");
  }
  auto collected = std::make_shared<Collected>(std::move(invoker));
  auto registry = std::make_shared<Registry>(
      cleanup.getObject(rt).getFunction(rt), collected);
  collected->registry = registry;

  jsi::Object obj(rt);
  obj.setProperty(
      rt, "register",
      jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forAscii(rt, "register"), 2,
          [registry](jsi::Runtime &rt, const jsi::Value &,
                     const jsi::Value *args, size_t count) -> jsi::Value {
            const jsi::Value undefined;
            registry->registerTarget(rt, count > 0 ? args[0] : undefined,
                                     count > 1 ? args[1] : undefined,
                                     count > 2 ? args[2] : undefined);
            return jsi::Value::undefined();
          }));
  obj.setProperty(
      rt, "unregister",
      jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forAscii(rt, "unregister"), 1,
          [registry](jsi::Runtime &rt, const jsi::Value &,
                     const jsi::Value *args, size_t count) -> jsi::Value {
            const jsi::Value undefined;
            return jsi::Value(
                registry->unregister(rt, count > 0 ? args[0] : undefined));
          }));
  return jsi::Value(rt, obj);
}

} // namespace ubrn_fin
