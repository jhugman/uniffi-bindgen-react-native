import { test } from "node:test";
import assert from "node:assert";
import { join } from "node:path";
import lib from "../lib.js";
const { UniffiNativeModule, FfiType } = lib;
import { pollUntil } from "./helpers/poll.mjs";

const LIB_PATH = join(
  import.meta.dirname,
  "..",
  "fixtures",
  "test_lib",
  "target",
  "debug",
  process.platform === "darwin"
    ? "libuniffi_napi_test_lib.dylib"
    : "libuniffi_napi_test_lib.so",
);

const SYMBOLS = {
  rustbufferAlloc: "uniffi_test_rustbuffer_alloc",
  rustbufferFree: "uniffi_test_rustbuffer_free",
  rustbufferFromBytes: "uniffi_test_rustbuffer_from_bytes",
};

function openLib() {
  return UniffiNativeModule.open(LIB_PATH);
}

test("callback: same-thread invocation", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {
      simple_callback: {
        args: [FfiType.UInt64, FfiType.Int8],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    functions: {
      uniffi_test_fn_call_callback: {
        args: [
          FfiType.Callback("simple_callback"),
          FfiType.UInt64,
          FfiType.Int8,
        ],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
  });

  let receivedHandle = null;
  let receivedValue = null;
  const callback = (handle, value) => {
    receivedHandle = handle;
    receivedValue = value;
  };

  const status = { code: 0 };
  nm.uniffi_test_fn_call_callback(callback, 42n, 7, status);

  assert.strictEqual(status.code, 0);
  assert.strictEqual(receivedHandle, 42n);
  assert.strictEqual(receivedValue, 7);
});

test("callback: receives RustBuffer arg as Uint8Array (same-thread)", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {},
    callbacks: {
      buffer_callback: {
        args: [FfiType.UInt64, FfiType.RustBuffer],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    functions: {
      uniffi_test_fn_call_callback_with_buffer: {
        args: [FfiType.Callback("buffer_callback"), FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
  });

  let receivedHandle = null;
  let receivedData = null;
  const callback = (handle, data) => {
    receivedHandle = handle;
    receivedData = data;
  };

  const status = { code: 0 };
  nm.uniffi_test_fn_call_callback_with_buffer(callback, 42n, status);

  assert.strictEqual(status.code, 0);
  assert.strictEqual(receivedHandle, 42n);
  assert.ok(receivedData instanceof Uint8Array);
  assert.deepStrictEqual(
    receivedData,
    new Uint8Array([0xde, 0xad, 0xbe, 0xef]),
  );
});

test("VTable: register struct with callbacks, call through", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {
      TestVTable: [
        { name: "get_value", type: FfiType.Callback("vtable_get_value") },
        { name: "free", type: FfiType.Callback("vtable_free") },
      ],
    },
    callbacks: {
      vtable_get_value: {
        args: [FfiType.UInt64],
        ret: FfiType.Int32,
        hasRustCallStatus: true,
      },
      vtable_free: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
    functions: {
      uniffi_test_fn_init_vtable: {
        args: [FfiType.Reference(FfiType.Struct("TestVTable"))],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_use_vtable: {
        args: [FfiType.UInt64],
        ret: FfiType.Int32,
        hasRustCallStatus: true,
      },
    },
  });

  // Register VTable with JS callbacks
  const status1 = { code: 0 };
  nm.uniffi_test_fn_init_vtable(
    {
      get_value: (handle, callStatus) => {
        callStatus.code = 0;
        return Number(handle) * 10;
      },
      free: (handle, callStatus) => {
        callStatus.code = 0;
      },
    },
    status1,
  );
  assert.strictEqual(status1.code, 0);

  // Call through the VTable
  const status2 = { code: 0 };
  const result = nm.uniffi_test_fn_use_vtable(7n, status2);
  assert.strictEqual(result, 70);
});

test("VTable: callback invoked from another thread returns value", async () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {
      TestVTable: [
        { name: "get_value", type: FfiType.Callback("vtable_get_value") },
        { name: "free", type: FfiType.Callback("vtable_free") },
      ],
    },
    callbacks: {
      vtable_get_value: {
        args: [FfiType.UInt64],
        ret: FfiType.Int32,
        hasRustCallStatus: true,
      },
      vtable_free: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
    functions: {
      uniffi_test_fn_init_vtable: {
        args: [FfiType.Reference(FfiType.Struct("TestVTable"))],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_use_vtable_from_thread: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_is_thread_done: {
        args: [],
        ret: FfiType.Int8,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_get_thread_result: {
        args: [],
        ret: FfiType.Int32,
        hasRustCallStatus: true,
      },
    },
  });

  // Register VTable with JS callbacks
  const status1 = { code: 0 };
  nm.uniffi_test_fn_init_vtable(
    {
      get_value: (handle, callStatus) => {
        callStatus.code = 0;
        return Number(handle) * 10;
      },
      free: (handle, callStatus) => {
        callStatus.code = 0;
      },
    },
    status1,
  );
  assert.strictEqual(status1.code, 0);

  // Fire off the cross-thread VTable call (returns immediately)
  const status2 = { code: 0 };
  nm.uniffi_test_fn_use_vtable_from_thread(7n, status2);
  assert.strictEqual(status2.code, 0);

  // Yield to event loop so TSF callback can fire, then poll for completion
  await pollUntil(
    () => nm.uniffi_test_fn_is_thread_done({ code: 0 }) === 1,
    "Timed out waiting for cross-thread VTable callback",
  );

  // Check the result
  const status3 = { code: 0 };
  const result = nm.uniffi_test_fn_get_thread_result(status3);
  assert.strictEqual(status3.code, 0);
  assert.strictEqual(result, 70); // 7 * 10
});

test("VTable: non-blocking callback invoked from another thread (fire-and-forget)", async () => {
  const lib = openLib();
  let notifiedHandle = null;

  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {
      NotifyVTable: [
        { name: "notify", type: FfiType.Callback("vtable_notify") },
      ],
    },
    callbacks: {
      vtable_notify: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: false,
      },
    },
    functions: {
      uniffi_test_fn_init_notify_vtable: {
        args: [FfiType.Reference(FfiType.Struct("NotifyVTable"))],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_notify_from_thread: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
  });

  const status1 = { code: 0 };
  nm.uniffi_test_fn_init_notify_vtable(
    {
      notify: (handle) => {
        notifiedHandle = handle;
      },
    },
    status1,
  );
  assert.strictEqual(status1.code, 0);

  // Fire off the non-blocking cross-thread call
  const status2 = { code: 0 };
  nm.uniffi_test_fn_notify_from_thread(42n, status2);
  assert.strictEqual(status2.code, 0);

  // Poll on the JS-side effect directly (not the Rust-side NOTIFY_DONE flag).
  await pollUntil(
    () => notifiedHandle !== null,
    "Timed out waiting for non-blocking VTable callback",
  );

  assert.strictEqual(notifiedHandle, 42n);
});

test("VTable: callback receives RustBuffer arg (same-thread)", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {
      BufferProcessorVTable: [
        { name: "process", type: FfiType.Callback("vtable_process") },
        { name: "free", type: FfiType.Callback("vtable_buf_free") },
      ],
    },
    callbacks: {
      vtable_process: {
        args: [FfiType.UInt64, FfiType.RustBuffer],
        ret: FfiType.UInt32,
        hasRustCallStatus: true,
      },
      vtable_buf_free: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
    functions: {
      uniffi_test_fn_init_buffer_vtable: {
        args: [FfiType.Reference(FfiType.Struct("BufferProcessorVTable"))],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_use_buffer_vtable: {
        args: [FfiType.UInt64],
        ret: FfiType.UInt32,
        hasRustCallStatus: true,
      },
    },
  });

  const status1 = { code: 0 };
  nm.uniffi_test_fn_init_buffer_vtable(
    {
      process: (handle, data, callStatus) => {
        callStatus.code = 0;
        // data should be Uint8Array [1, 2, 3, 4, 5]
        // Return the sum of bytes as u32
        let sum = 0;
        for (const b of data) sum += b;
        return sum;
      },
      free: (handle, callStatus) => {
        callStatus.code = 0;
      },
    },
    status1,
  );
  assert.strictEqual(status1.code, 0);

  const status2 = { code: 0 };
  const result = nm.uniffi_test_fn_use_buffer_vtable(1n, status2);
  assert.strictEqual(status2.code, 0);
  assert.strictEqual(result, 15); // 1+2+3+4+5
});

test("VTable: callback returns RustBuffer (same-thread)", () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {
      BufferReturnerVTable: [
        { name: "get_data", type: FfiType.Callback("vtable_get_data") },
        { name: "free", type: FfiType.Callback("vtable_ret_free") },
      ],
    },
    callbacks: {
      vtable_get_data: {
        args: [FfiType.UInt64],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
      vtable_ret_free: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
    functions: {
      uniffi_test_fn_init_buffer_returner_vtable: {
        args: [FfiType.Reference(FfiType.Struct("BufferReturnerVTable"))],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_use_buffer_returner: {
        args: [FfiType.UInt64],
        ret: FfiType.UInt32,
        hasRustCallStatus: true,
      },
    },
  });

  const status1 = { code: 0 };
  nm.uniffi_test_fn_init_buffer_returner_vtable(
    {
      get_data: (handle, callStatus) => {
        callStatus.code = 0;
        // Return a Uint8Array — should be converted to RustBuffer
        return new Uint8Array([10, 20, 30, 40]);
      },
      free: (handle, callStatus) => {
        callStatus.code = 0;
      },
    },
    status1,
  );
  assert.strictEqual(status1.code, 0);

  const status2 = { code: 0 };
  const result = nm.uniffi_test_fn_use_buffer_returner(1n, status2);
  assert.strictEqual(status2.code, 0);
  assert.strictEqual(result, 100); // 10+20+30+40
});

test("VTable: callback returns RustBuffer from another thread", async () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {
      BufferReturnerVTable: [
        { name: "get_data", type: FfiType.Callback("vtable_get_data") },
        { name: "free", type: FfiType.Callback("vtable_ret_free") },
      ],
    },
    callbacks: {
      vtable_get_data: {
        args: [FfiType.UInt64],
        ret: FfiType.RustBuffer,
        hasRustCallStatus: true,
      },
      vtable_ret_free: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
    functions: {
      uniffi_test_fn_init_buffer_returner_vtable: {
        args: [FfiType.Reference(FfiType.Struct("BufferReturnerVTable"))],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_use_buffer_returner_from_thread: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_is_returner_thread_done: {
        args: [],
        ret: FfiType.Int8,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_get_returner_thread_result: {
        args: [],
        ret: FfiType.Int32,
        hasRustCallStatus: true,
      },
    },
  });

  const status1 = { code: 0 };
  nm.uniffi_test_fn_init_buffer_returner_vtable(
    {
      get_data: (handle, callStatus) => {
        callStatus.code = 0;
        return new Uint8Array([5, 10, 15, 20, 25]);
      },
      free: (handle, callStatus) => {
        callStatus.code = 0;
      },
    },
    status1,
  );
  assert.strictEqual(status1.code, 0);

  const status2 = { code: 0 };
  nm.uniffi_test_fn_use_buffer_returner_from_thread(1n, status2);
  assert.strictEqual(status2.code, 0);

  await pollUntil(
    () => nm.uniffi_test_fn_is_returner_thread_done({ code: 0 }) === 1,
  );

  const status3 = { code: 0 };
  const result = nm.uniffi_test_fn_get_returner_thread_result(status3);
  assert.strictEqual(status3.code, 0);
  assert.strictEqual(result, 75); // 5+10+15+20+25
});

test("VTable: callback receives RustBuffer arg from another thread", async () => {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: {
      BufferProcessorVTable: [
        { name: "process", type: FfiType.Callback("vtable_process") },
        { name: "free", type: FfiType.Callback("vtable_buf_free") },
      ],
    },
    callbacks: {
      vtable_process: {
        args: [FfiType.UInt64, FfiType.RustBuffer],
        ret: FfiType.UInt32,
        hasRustCallStatus: true,
      },
      vtable_buf_free: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
    },
    functions: {
      uniffi_test_fn_init_buffer_vtable: {
        args: [FfiType.Reference(FfiType.Struct("BufferProcessorVTable"))],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_use_buffer_vtable_from_thread: {
        args: [FfiType.UInt64],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_is_buffer_thread_done: {
        args: [],
        ret: FfiType.Int8,
        hasRustCallStatus: true,
      },
      uniffi_test_fn_get_buffer_thread_result: {
        args: [],
        ret: FfiType.Int32,
        hasRustCallStatus: true,
      },
    },
  });

  const status1 = { code: 0 };
  nm.uniffi_test_fn_init_buffer_vtable(
    {
      process: (handle, data, callStatus) => {
        callStatus.code = 0;
        // data should be Uint8Array [10, 20, 30]
        let sum = 0;
        for (const b of data) sum += b;
        return sum;
      },
      free: (handle, callStatus) => {
        callStatus.code = 0;
      },
    },
    status1,
  );
  assert.strictEqual(status1.code, 0);

  const status2 = { code: 0 };
  nm.uniffi_test_fn_use_buffer_vtable_from_thread(1n, status2);
  assert.strictEqual(status2.code, 0);

  await pollUntil(
    () => nm.uniffi_test_fn_is_buffer_thread_done({ code: 0 }) === 1,
  );

  const status3 = { code: 0 };
  const result = nm.uniffi_test_fn_get_buffer_thread_result(status3);
  assert.strictEqual(status3.code, 0);
  assert.strictEqual(result, 60); // 10+20+30
});

// --- Shared ScalarEchoVTable registration definitions ---

const SCALAR_ECHO_VTABLE_STRUCT = [
  { name: "echo_u8", type: FfiType.Callback("vt_echo_u8") },
  { name: "echo_i8", type: FfiType.Callback("vt_echo_i8") },
  { name: "echo_u16", type: FfiType.Callback("vt_echo_u16") },
  { name: "echo_i16", type: FfiType.Callback("vt_echo_i16") },
  { name: "echo_u32", type: FfiType.Callback("vt_echo_u32") },
  { name: "echo_i32", type: FfiType.Callback("vt_echo_i32") },
  { name: "echo_u64", type: FfiType.Callback("vt_echo_u64") },
  { name: "echo_i64", type: FfiType.Callback("vt_echo_i64") },
  { name: "echo_f32", type: FfiType.Callback("vt_echo_f32") },
  { name: "echo_f64", type: FfiType.Callback("vt_echo_f64") },
  { name: "free", type: FfiType.Callback("vt_echo_free") },
];

const SCALAR_ECHO_CALLBACKS = {
  vt_echo_u8: {
    args: [FfiType.UInt64, FfiType.UInt8],
    ret: FfiType.UInt8,
    hasRustCallStatus: true,
  },
  vt_echo_i8: {
    args: [FfiType.UInt64, FfiType.Int8],
    ret: FfiType.Int8,
    hasRustCallStatus: true,
  },
  vt_echo_u16: {
    args: [FfiType.UInt64, FfiType.UInt16],
    ret: FfiType.UInt16,
    hasRustCallStatus: true,
  },
  vt_echo_i16: {
    args: [FfiType.UInt64, FfiType.Int16],
    ret: FfiType.Int16,
    hasRustCallStatus: true,
  },
  vt_echo_u32: {
    args: [FfiType.UInt64, FfiType.UInt32],
    ret: FfiType.UInt32,
    hasRustCallStatus: true,
  },
  vt_echo_i32: {
    args: [FfiType.UInt64, FfiType.Int32],
    ret: FfiType.Int32,
    hasRustCallStatus: true,
  },
  vt_echo_u64: {
    args: [FfiType.UInt64, FfiType.UInt64],
    ret: FfiType.UInt64,
    hasRustCallStatus: true,
  },
  vt_echo_i64: {
    args: [FfiType.UInt64, FfiType.Int64],
    ret: FfiType.Int64,
    hasRustCallStatus: true,
  },
  vt_echo_f32: {
    args: [FfiType.UInt64, FfiType.Float32],
    ret: FfiType.Float32,
    hasRustCallStatus: true,
  },
  vt_echo_f64: {
    args: [FfiType.UInt64, FfiType.Float64],
    ret: FfiType.Float64,
    hasRustCallStatus: true,
  },
  vt_echo_free: {
    args: [FfiType.UInt64],
    ret: FfiType.Void,
    hasRustCallStatus: true,
  },
};

function registerScalarEchoVTable(extraFunctions = {}) {
  const lib = openLib();
  const nm = lib.register({
    symbols: SYMBOLS,
    structs: { ScalarEchoVTable: SCALAR_ECHO_VTABLE_STRUCT },
    callbacks: SCALAR_ECHO_CALLBACKS,
    functions: {
      uniffi_test_fn_init_scalar_echo_vtable: {
        args: [FfiType.Reference(FfiType.Struct("ScalarEchoVTable"))],
        ret: FfiType.Void,
        hasRustCallStatus: true,
      },
      ...extraFunctions,
    },
  });

  const echo = (handle, value, callStatus) => {
    callStatus.code = 0;
    return value;
  };
  const status0 = { code: 0 };
  nm.uniffi_test_fn_init_scalar_echo_vtable(
    {
      echo_u8: echo,
      echo_i8: echo,
      echo_u16: echo,
      echo_i16: echo,
      echo_u32: echo,
      echo_i32: echo,
      echo_u64: echo,
      echo_i64: echo,
      echo_f32: echo,
      echo_f64: echo,
      free: (handle, callStatus) => {
        callStatus.code = 0;
      },
    },
    status0,
  );
  assert.strictEqual(status0.code, 0);

  return nm;
}

test("VTable: scalar echo \u2014 all types round-trip (same-thread)", () => {
  const nm = registerScalarEchoVTable({
    uniffi_test_fn_echo_u8_via_vtable: {
      args: [FfiType.UInt64, FfiType.UInt8],
      ret: FfiType.UInt8,
      hasRustCallStatus: true,
    },
    uniffi_test_fn_echo_i8_via_vtable: {
      args: [FfiType.UInt64, FfiType.Int8],
      ret: FfiType.Int8,
      hasRustCallStatus: true,
    },
    uniffi_test_fn_echo_u16_via_vtable: {
      args: [FfiType.UInt64, FfiType.UInt16],
      ret: FfiType.UInt16,
      hasRustCallStatus: true,
    },
    uniffi_test_fn_echo_i16_via_vtable: {
      args: [FfiType.UInt64, FfiType.Int16],
      ret: FfiType.Int16,
      hasRustCallStatus: true,
    },
    uniffi_test_fn_echo_u32_via_vtable: {
      args: [FfiType.UInt64, FfiType.UInt32],
      ret: FfiType.UInt32,
      hasRustCallStatus: true,
    },
    uniffi_test_fn_echo_i32_via_vtable: {
      args: [FfiType.UInt64, FfiType.Int32],
      ret: FfiType.Int32,
      hasRustCallStatus: true,
    },
    uniffi_test_fn_echo_u64_via_vtable: {
      args: [FfiType.UInt64, FfiType.UInt64],
      ret: FfiType.UInt64,
      hasRustCallStatus: true,
    },
    uniffi_test_fn_echo_i64_via_vtable: {
      args: [FfiType.UInt64, FfiType.Int64],
      ret: FfiType.Int64,
      hasRustCallStatus: true,
    },
    uniffi_test_fn_echo_f32_via_vtable: {
      args: [FfiType.UInt64, FfiType.Float32],
      ret: FfiType.Float32,
      hasRustCallStatus: true,
    },
    uniffi_test_fn_echo_f64_via_vtable: {
      args: [FfiType.UInt64, FfiType.Float64],
      ret: FfiType.Float64,
      hasRustCallStatus: true,
    },
  });

  const s = () => ({ code: 0 });

  // Integer types — including boundary values
  assert.strictEqual(nm.uniffi_test_fn_echo_u8_via_vtable(1n, 0, s()), 0);
  assert.strictEqual(nm.uniffi_test_fn_echo_u8_via_vtable(1n, 255, s()), 255);
  assert.strictEqual(nm.uniffi_test_fn_echo_i8_via_vtable(1n, -128, s()), -128);
  assert.strictEqual(nm.uniffi_test_fn_echo_i8_via_vtable(1n, 127, s()), 127);
  assert.strictEqual(nm.uniffi_test_fn_echo_u16_via_vtable(1n, 0, s()), 0);
  assert.strictEqual(
    nm.uniffi_test_fn_echo_u16_via_vtable(1n, 65535, s()),
    65535,
  );
  assert.strictEqual(
    nm.uniffi_test_fn_echo_i16_via_vtable(1n, -32768, s()),
    -32768,
  );
  assert.strictEqual(
    nm.uniffi_test_fn_echo_i16_via_vtable(1n, 32767, s()),
    32767,
  );
  assert.strictEqual(nm.uniffi_test_fn_echo_u32_via_vtable(1n, 0, s()), 0);
  assert.strictEqual(
    nm.uniffi_test_fn_echo_u32_via_vtable(1n, 4294967295, s()),
    4294967295,
  );
  assert.strictEqual(
    nm.uniffi_test_fn_echo_i32_via_vtable(1n, -2147483648, s()),
    -2147483648,
  );
  assert.strictEqual(
    nm.uniffi_test_fn_echo_i32_via_vtable(1n, 2147483647, s()),
    2147483647,
  );
  assert.strictEqual(nm.uniffi_test_fn_echo_u64_via_vtable(1n, 0n, s()), 0n);
  assert.strictEqual(
    nm.uniffi_test_fn_echo_u64_via_vtable(1n, 9007199254740993n, s()),
    9007199254740993n,
  );
  assert.strictEqual(
    nm.uniffi_test_fn_echo_i64_via_vtable(1n, -9007199254740993n, s()),
    -9007199254740993n,
  );
  assert.strictEqual(nm.uniffi_test_fn_echo_i64_via_vtable(1n, 0n, s()), 0n);

  // Floating point — including boundary values
  const f32Result = nm.uniffi_test_fn_echo_f32_via_vtable(1n, 3.14, s());
  assert.ok(
    Math.abs(f32Result - 3.14) < 0.01,
    `f32 echo: expected ~3.14, got ${f32Result}`,
  );
  assert.strictEqual(nm.uniffi_test_fn_echo_f32_via_vtable(1n, 0, s()), 0);
  assert.strictEqual(
    nm.uniffi_test_fn_echo_f64_via_vtable(1n, 2.718281828, s()),
    2.718281828,
  );
  assert.strictEqual(nm.uniffi_test_fn_echo_f64_via_vtable(1n, 0, s()), 0);
  assert.strictEqual(nm.uniffi_test_fn_echo_f64_via_vtable(1n, -0, s()), -0);
  assert.strictEqual(
    nm.uniffi_test_fn_echo_f64_via_vtable(1n, Number.MAX_SAFE_INTEGER, s()),
    Number.MAX_SAFE_INTEGER,
  );
});

test("VTable: scalar echo \u2014 all types round-trip (cross-thread)", async () => {
  const nm = registerScalarEchoVTable({
    uniffi_test_fn_echo_all_scalars_via_vtable_from_thread: {
      args: [FfiType.UInt64],
      ret: FfiType.Void,
      hasRustCallStatus: true,
    },
    uniffi_test_fn_is_scalar_echo_thread_done: {
      args: [],
      ret: FfiType.Int8,
      hasRustCallStatus: true,
    },
    uniffi_test_fn_get_scalar_echo_thread_result: {
      args: [],
      ret: FfiType.Int32,
      hasRustCallStatus: true,
    },
  });

  // Fire cross-thread echo of all scalar types
  const status1 = { code: 0 };
  nm.uniffi_test_fn_echo_all_scalars_via_vtable_from_thread(1n, status1);
  assert.strictEqual(status1.code, 0);

  // Poll for completion
  await pollUntil(
    () => nm.uniffi_test_fn_is_scalar_echo_thread_done({ code: 0 }) === 1,
  );

  // All 10 types should have echoed correctly
  const status2 = { code: 0 };
  const passCount = nm.uniffi_test_fn_get_scalar_echo_thread_result(status2);
  assert.strictEqual(status2.code, 0);
  assert.strictEqual(
    passCount,
    10,
    `Expected 10 scalar types to pass, got ${passCount}`,
  );
});
