/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Async runtime for polling UniFFI Rust futures.
//
// UniFFI async functions return a future handle (u64). The caller must
// poll the future with a continuation callback until it signals READY,
// then call complete() to extract the result and free() to drop the handle.
//
// The continuation callback is a fire-and-forget (void, no RustCallStatus)
// function invoked by Rust — possibly from a worker thread — with
// (handle: u64, pollResult: i8). We use a HandleMap to correlate each
// poll call with its Promise resolver.

const POLL_READY = 0;

/**
 * Maps bigint handles to Promise resolve functions.
 * Each in-flight poll gets a unique handle; the continuation callback
 * looks up the resolver by handle and calls it with the poll result.
 */
class HandleMap {
  #nextHandle = 1n;
  #map = new Map();

  insert(value) {
    const handle = this.#nextHandle;
    this.#nextHandle += 1n;
    this.#map.set(handle, value);
    return handle;
  }

  remove(handle) {
    const value = this.#map.get(handle);
    this.#map.delete(handle);
    return value;
  }
}

const resolverMap = new HandleMap();

/**
 * The continuation callback passed to Rust's poll function.
 * Signature at the C level: (handle: u64, pollResult: i8) -> void.
 * hasRustCallStatus: false (fire-and-forget).
 *
 * When Rust finishes a poll iteration, it calls this from whichever
 * thread the future was polled on. uniffi-runtime-napi's NonBlocking TSF
 * dispatch delivers the call to the main thread's event loop.
 */
function continuationCallback(handle, pollResult) {
  const resolve = resolverMap.remove(handle);
  if (resolve) resolve(pollResult);
}

/**
 * Poll a UniFFI Rust future to completion and return the result.
 *
 * @param {object} nm - The registered native module (from register())
 * @param {object} opts
 * @param {function} opts.rustFutureFunc - () => bigint: initiates the async call, returns future handle
 * @param {string} opts.pollFunc - Symbol name for the poll function
 * @param {string} opts.completeFunc - Symbol name for the complete function
 * @param {string} opts.freeFunc - Symbol name for the free function
 * @param {function} [opts.liftFunc] - (rawResult) => T: converts the raw return value to JS
 * @param {object} [opts.callStatus] - { code: 0 } RustCallStatus object for complete()
 * @returns {Promise<*>} The lifted result
 */
async function uniffiRustCallAsync(
  nm,
  { rustFutureFunc, pollFunc, completeFunc, freeFunc, liftFunc, callStatus },
) {
  const futureHandle = rustFutureFunc();
  const status = callStatus || { code: 0 };

  try {
    let pollResult;
    do {
      pollResult = await new Promise((resolve) => {
        const handle = resolverMap.insert(resolve);
        nm[pollFunc](futureHandle, continuationCallback, handle);
      });
    } while (pollResult !== POLL_READY);

    const result = nm[completeFunc](futureHandle, status);
    if (status.code !== 0) {
      throw new Error(`Rust async call failed with code ${status.code}`);
    }
    return liftFunc ? liftFunc(result) : result;
  } finally {
    nm[freeFunc](futureHandle);
  }
}

export {
  POLL_READY,
  HandleMap,
  resolverMap,
  continuationCallback,
  uniffiRustCallAsync,
};
