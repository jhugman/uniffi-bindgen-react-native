/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

#include <chrono>
#include <jsi/jsi.h>
#include <memory>

namespace facebook::react {

enum class SchedulerPriority : int {
  ImmediatePriority = 1,
  UserBlockingPriority = 2,
  NormalPriority = 3,
  LowPriority = 4,
  IdlePriority = 5,
};

using CallFunc = std::function<void(jsi::Runtime &)>;

/**
 * An interface for a generic native-to-JS call invoker. See BridgeJSCallInvoker
 * for an implementation.
 */
class CallInvoker {
public:
  virtual void invokeAsync(CallFunc &&func) noexcept = 0;
  virtual void invokeAsync(SchedulerPriority /*priority*/,
                           CallFunc &&func) noexcept {
    // When call with priority is not implemented, fall back to a regular async
    // execution
    invokeAsync(std::move(func));
  }
  virtual void invokeSync(CallFunc &&func) = 0;

  // Backward compatibility only, prefer the CallFunc methods instead
  virtual void invokeAsync(std::function<void()> &&func) noexcept {
    invokeAsync([func](jsi::Runtime &) { func(); });
  }

  virtual void invokeSync(std::function<void()> &&func) {
    invokeSync([func](jsi::Runtime &) { func(); });
  }

  virtual ~CallInvoker() {}
};

using NativeMethodCallFunc = std::function<void()>;

class NativeMethodCallInvoker {
public:
  virtual void invokeAsync(const std::string &methodName,
                           NativeMethodCallFunc &&func) noexcept = 0;
  virtual void invokeSync(const std::string &methodName,
                          NativeMethodCallFunc &&func) = 0;
  virtual ~NativeMethodCallInvoker() {}
};
} // namespace facebook::react
