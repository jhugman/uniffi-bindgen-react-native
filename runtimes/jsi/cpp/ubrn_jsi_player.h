/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

#include <ReactCommon/CallInvoker.h>
#include <jsi/jsi.h>

#include <functional>
#include <memory>
#include <string>

namespace ubrn::jsi_player {

// Maps the `name` a generated binding passes to `uniffi.open({ name })` onto
// the path the player hands to dlopen. Each host knows its own layout: a
// soname on Android, an embedded framework on iOS, a directory on the host.
using Resolver = std::function<std::string(const std::string &name)>;

// Installs `globalThis.uniffi` on `rt`. Call once per JS runtime.
void install(facebook::jsi::Runtime &rt,
             std::shared_ptr<facebook::react::CallInvoker> callInvoker,
             Resolver resolver);

} // namespace ubrn::jsi_player
