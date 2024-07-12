/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#include <ReactCommon/CallInvoker.h>
#include <condition_variable>
#include <mutex>
#include <optional>
#include <queue>

namespace uniffi::testing {
class MyCallInvoker : public facebook::react::CallInvoker {
private:
  facebook::jsi::Runtime &runtime_;
  std::queue<facebook::react::CallFunc> tasks_;
  mutable std::mutex queueMutex_;
  std::condition_variable newTaskCond_;

public:
  MyCallInvoker(facebook::jsi::Runtime &runtime) : runtime_(runtime) {}

  void invokeAsync(facebook::react::CallFunc &&func) noexcept override {
    {
      std::lock_guard<std::mutex> lock(queueMutex_);
      tasks_.push(std::move(func));
    }
    newTaskCond_.notify_one();
  }

  void invokeSync(facebook::react::CallFunc &&func) override {
    // Implement this method with your own logic
    // You can use runtime_ here
  }

  std::optional<facebook::react::CallFunc> nextTask() {
    std::lock_guard<std::mutex> lock(queueMutex_);
    if (tasks_.empty()) {
      return std::nullopt; // Return an empty optional
    }
    auto task = std::move(tasks_.front());
    tasks_.pop();
    return task; // Return the task wrapped in an optional
  }

  bool waitForTaskOrTimeout(double duration) {
    auto timeout = std::chrono::milliseconds((int_least64_t)duration);
    std::unique_lock<std::mutex> lock(queueMutex_);
    newTaskCond_.wait_for(lock, timeout, [this] { return !tasks_.empty(); });
    return !tasks_.empty();
  }

  bool isEmpty() const {
    std::lock_guard<std::mutex> lock(queueMutex_);
    return tasks_.empty();
  }

  void drainTasks(facebook::jsi::Runtime &runtime) {
    while (true) {
      auto optionalFunc = this->nextTask();
      if (!optionalFunc.has_value()) {
        return;
      }
      auto &func = *optionalFunc;
      func(runtime);
    }
  }

  ~MyCallInvoker() override {}
};
} // namespace uniffi::testing
