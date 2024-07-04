/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once
#include <ReactCommon/CallInvoker.h>
#include <future>
#include <memory>
#include <thread>

namespace uniffi_runtime {
namespace jsi = facebook::jsi;
namespace react = facebook::react;
using CallFunc = std::function<void(jsi::Runtime &)>;

/**
 * A wrapper class to invoke a callback function on the JS thread.
 *
 * The is intended to have two methods: `invokeSync(func)` and
 * `invokeAsync(func)`, one for invoking a callback that should return on the
 * same thread it was invoked upon, and the other for invoking the callback and
 * returning a promise.
 */
class UniffiCallInvoker {
private:
  std::shared_ptr<react::CallInvoker> callInvoker_;
  std::thread::id threadId_;

public:
  UniffiCallInvoker(std::shared_ptr<react::CallInvoker> callInvoker)
      : callInvoker_(callInvoker), threadId_(std::this_thread::get_id()) {}

  /**
   * Invokes the given function on the JS thread.
   *
   * If called from the JS thread, then the callback func is invoked
   * immediately.
   *
   * Otherwise, the callback is invoked on the JS thread, and this thread blocks
   * until it completes.
   */
  void invokeSync(jsi::Runtime &rt, CallFunc &func) {
    if (std::this_thread::get_id() == threadId_) {
      func(rt);
    } else {
      std::promise<void> promise;
      auto future = promise.get_future();
      callInvoker_->invokeAsync([&rt, &func, &promise]() {
        func(rt);
        promise.set_value();
      });
      future.wait();
    }
  }
};
} // namespace uniffi_runtime
