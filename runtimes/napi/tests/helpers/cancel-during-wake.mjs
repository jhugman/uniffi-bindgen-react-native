/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Cancels a future while a Rust thread is waking it, printing how the future completed.
// Run as a script by threading.test.mjs; see there for why it is a separate process.
import lib from "../../lib.js";
import { libPath } from "./lib-path.mjs";
import {
  legacyDefinitions,
  pollToReady,
  spinUntilWakerEntered,
} from "./test-lib-futures.mjs";

const { UniffiNativeModule } = lib;

const nm = UniffiNativeModule.open(libPath("uniffi_napi_test_lib")).register(
  legacyDefinitions(),
);

const status = { code: 0 };
const f = nm.uniffi_test_fn_future_wake_under_lock();
// The first poll spawns the waker. pollToReady's ref'd timer keeps Node alive until READY.
const ready = pollToReady(nm, f);
spinUntilWakerEntered(nm);
// The waker holds the lock cancel takes, across its continuation call.
nm.uniffi_test_rust_future_cancel(f);
// The waker took the continuation, so cancel fired nothing: the delivered one re-polls.
await ready;
nm.uniffi_test_rust_future_complete_u32(f, status);
console.log(`STATUS ${status.code}`);
nm.uniffi_test_rust_future_free(f);
console.log(`LIVE ${nm.uniffi_test_live_future_count({ code: 0 })}`);
