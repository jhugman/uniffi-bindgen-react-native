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
} // namespace uniffi_jsi
