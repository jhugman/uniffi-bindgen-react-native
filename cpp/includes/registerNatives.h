/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

#pragma once

#include <ReactCommon/CallInvoker.h>
#include <jsi/jsi.h>

/// Register host functions into the given runtime.
extern "C" void
registerNatives(facebook::jsi::Runtime &rt,
                std::shared_ptr<facebook::react::CallInvoker> callInvoker);
