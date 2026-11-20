/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Tables and a poll loop for test_lib's futures.
//
// The type tags are written out here, not taken from lib.js: tests that need the native
// `register` must not load lib.js.

const Int8 = { tag: "Int8" };
const UInt32 = { tag: "UInt32" };
const Int32 = { tag: "Int32" };
const UInt64 = { tag: "UInt64" };
const Handle = { tag: "Handle" };
const RustBuffer = { tag: "RustBuffer" };
const Void = { tag: "Void" };
const Callback = (name) => ({ tag: "Callback", name });

const SYMBOLS = {
  rustbuffer_alloc: "uniffi_test_rustbuffer_alloc",
  rustbuffer_free: "uniffi_test_rustbuffer_free",
  rustbuffer_from_bytes: "uniffi_test_rustbuffer_from_bytes",
};

export const CONTINUATION = "RustFutureContinuationCallback";

export const POLL = "uniffi_test_rust_future_poll";
export const CANCEL = "uniffi_test_rust_future_cancel";
export const COMPLETE_U32 = "uniffi_test_rust_future_complete_u32";
export const COMPLETE_RUST_BUFFER =
  "uniffi_test_rust_future_complete_rust_buffer";
export const FREE = "uniffi_test_rust_future_free";

// Start functions take no status and return the future's handle.
const STARTS = {
  uniffi_test_fn_future_u32: [UInt32],
  uniffi_test_fn_future_buffer: [UInt32],
  uniffi_test_fn_future_error: [],
  uniffi_test_fn_future_from_thread: [UInt32, UInt32],
  uniffi_test_fn_future_parked: [],
  uniffi_test_fn_future_wake_under_lock: [],
};

const PROBES = {
  uniffi_test_live_future_count: {
    args: [],
    ret: Int32,
    hasRustCallStatus: true,
  },
  uniffi_test_live_buffer_count: {
    args: [],
    ret: Int32,
    hasRustCallStatus: true,
  },
  uniffi_test_fn_waker_entered: {
    args: [],
    ret: Int8,
    hasRustCallStatus: true,
  },
};

function start(args) {
  return { args, ret: Handle, hasRustCallStatus: false };
}

// The future exports as a table names them for bindings that run the poll loop in JS.
function futureExports() {
  const handleOnly = (ret, hasRustCallStatus) => ({
    args: [Handle],
    ret,
    hasRustCallStatus,
  });
  return {
    [POLL]: {
      args: [Handle, Callback(CONTINUATION), Handle],
      ret: Void,
      hasRustCallStatus: false,
    },
    [CANCEL]: handleOnly(Void, false),
    [COMPLETE_U32]: handleOnly(UInt32, true),
    [COMPLETE_RUST_BUFFER]: handleOnly(RustBuffer, true),
    [FREE]: handleOnly(Void, false),
  };
}

export function legacyDefinitions() {
  const functions = {};
  for (const [name, args] of Object.entries(STARTS)) {
    functions[name] = start(args);
  }
  return {
    symbols: SYMBOLS,
    structs: {},
    callbacks: {
      [CONTINUATION]: {
        args: [UInt64, Int8],
        ret: Void,
        hasRustCallStatus: false,
      },
    },
    functions: { ...functions, ...futureExports(), ...PROBES },
  };
}

export function asyncDefinitions() {
  const functions = {};
  for (const [name, args] of Object.entries(STARTS)) {
    const [complete, ret] =
      name === "uniffi_test_fn_future_buffer"
        ? [COMPLETE_RUST_BUFFER, RustBuffer]
        : [COMPLETE_U32, UInt32];
    functions[name] = {
      ...start(args),
      async: { poll: POLL, cancel: CANCEL, complete, free: FREE, ret },
    };
  }
  return {
    symbols: SYMBOLS,
    structs: {},
    callbacks: {},
    functions: {
      ...functions,
      ...PROBES,
      uniffi_test_fn_buffer_len: {
        args: [RustBuffer],
        ret: UInt32,
        hasRustCallStatus: true,
      },
    },
  };
}

const POLL_READY = 0;

// One continuation for every poll, so the runtime builds one trampoline for it.
const resolvers = new Map();
let nextPoll = 1n;
function continuation(data, pollResult) {
  const resolve = resolvers.get(data);
  resolvers.delete(data);
  if (resolve) resolve(pollResult);
}

function pollOnce(nm, future) {
  return new Promise((resolve) => {
    const data = nextPoll++;
    resolvers.set(data, resolve);
    try {
      nm[POLL](future, continuation, data);
    } catch (e) {
      resolvers.delete(data);
      throw e;
    }
  });
}

// Polls `future` until READY. The first poll runs before this returns.
export async function pollToReady(nm, future) {
  // A continuation from another thread arrives through an unref'd threadsafe function, so
  // without a ref'd handle Node may exit before it lands.
  const keepAlive = setInterval(() => {}, 60_000);
  try {
    while ((await pollOnce(nm, future)) !== POLL_READY) {
      // MAYBE_READY: poll again.
    }
  } finally {
    clearInterval(keepAlive);
  }
}

// Spins until the under-lock waker holds its lock, exiting the process after 10 s.
export function spinUntilWakerEntered(nm) {
  const deadline = Date.now() + 10_000;
  while (nm.uniffi_test_fn_waker_entered({ code: 0 }) === 0) {
    if (Date.now() > deadline) {
      console.log("NO WAKER");
      process.exit(1);
    }
  }
}
