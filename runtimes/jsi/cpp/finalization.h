/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// finalization.h — the player's stand-in for FinalizationRegistry.
//
// Hermes has no FinalizationRegistry, and without one an object JS drops
// without calling uniffiDestroy() is never freed in Rust. The player offers one
// in its place, as `uniffi.createFinalizationRegistry(cleanup)`, rather than as
// a global, which would change what every other library in the app sees.
#pragma once
#include <ReactCommon/CallInvoker.h>
#include <jsi/jsi.h>

#include <memory>

namespace ubrn_fin {

// A registry with FinalizationRegistry's `register(target, heldValue,
// unregisterToken?)` and `unregister(unregisterToken)`. A factory and not a
// constructor: Hermes cannot `new` a host function.
//
// Each target carries a native destructor. That destructor only queues the
// registration, because it runs inside the collector, possibly off the JS
// thread or as the runtime is destroyed, where no JS may run. A task posted
// through `callInvoker` later hands each queued held value to `cleanup` on the
// JS thread.
//
// A target or token that already carries another native state cannot take a
// destructor, so `register` throws a TypeError for one.
facebook::jsi::Value createFinalizationRegistry(
    facebook::jsi::Runtime &rt,
    std::shared_ptr<facebook::react::CallInvoker> callInvoker,
    const facebook::jsi::Value &cleanup);

} // namespace ubrn_fin
