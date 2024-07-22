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
 * The is intended to have two methods:
 *
 * 1. `invokeBlocking`, which will wait until the JS thread is available,
 *    call the callback function, and then return.
 * 2. `invokeNonBlocking`, which will schedule the callback function to be
 * called when the JS thread is available, but not wait for it to complete.
 *
 * Conceptually, the `invokeNonBlocking` method should be more useful than it
 * actually is, however: the generated C++ cannot easily tell if the callback
 * function is synchronous or not.
 *
 * Other optimizations might also be available to use the `invokeNonBlocking`
 * method (e.g. `void` returns), were it not for the error cases.
 *
 * Until we can tell if the callback is async, we always use `invokeBlocking`,
 * and leave `invokeNonBlocking` unimplemented.
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
  void invokeBlocking(jsi::Runtime &rt, CallFunc &func) {
    if (std::this_thread::get_id() == threadId_) {
      func(rt);
    } else {
      std::promise<void> promise;
      auto future = promise.get_future();
      // The runtime argument was added to CallFunc in
      // https://github.com/facebook/react-native/pull/43375
      //
      // Once that is released, there will be a deprecation period.
      //
      // Any time during the deprecation period, we can switch `&rt`
      // from being a captured variable to being an argument, i.e.
      // commenting out one line, and uncommenting the other.
      std::function<void()> wrapper = [&func, &promise, &rt]() {
        // react::CallFunc wrapper = [&func, &promise](jsi::Runtime &rt) {
        func(rt);
        promise.set_value();
      };
      callInvoker_->invokeAsync(std::move(wrapper));
      future.wait();
    }
  }
};
} // namespace uniffi_runtime
