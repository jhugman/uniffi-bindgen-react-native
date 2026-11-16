/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Aborts an async entry while a Rust thread is waking its future, printing how it completed.
// Run as a script by threading.test.mjs; see there for why it is a separate process.
import lib from "../../lib.js";
import { libPath } from "./lib-path.mjs";
import {
  asyncDefinitions,
  spinUntilWakerEntered,
} from "./test-lib-futures.mjs";

const { UniffiNativeModule } = lib;

const nm = UniffiNativeModule.open(libPath("uniffi_napi_test_lib")).register(
  asyncDefinitions(),
);

const status = { code: 0 };
const c = new AbortController();
// The entry's first poll spawns the waker. The drive's own keep-alive timer holds Node open.
const p = nm.uniffi_test_fn_future_wake_under_lock(status, c.signal);
spinUntilWakerEntered(nm);
// The abort listener calls cancel, which takes the lock the waker holds.
c.abort();
await p;
console.log(`STATUS ${status.code}`);
console.log(`LIVE ${nm.uniffi_test_live_future_count({ code: 0 })}`);
