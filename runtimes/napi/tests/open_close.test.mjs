import { test } from "node:test";
import assert from "node:assert";
import { join } from "node:path";
import lib from "../lib.js";
const { UniffiNativeModule } = lib;

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

test("open() loads a library", () => {
  const lib = UniffiNativeModule.open(LIB_PATH);
  assert.ok(lib);
});

test("open() throws for nonexistent library", () => {
  assert.throws(() => {
    UniffiNativeModule.open("/nonexistent/lib.dylib");
  }, /Error/);
});

test("register() throws for missing symbol", () => {
  const lib = UniffiNativeModule.open(LIB_PATH);
  assert.throws(() => {
    lib.register({
      symbols: {
        rustbufferAlloc: "nonexistent_symbol",
        rustbufferFree: "uniffi_test_rustbuffer_free",
        rustbufferFromBytes: "uniffi_test_rustbuffer_from_bytes",
      },
      structs: {},
      callbacks: {},
      functions: {},
    });
  }, /Error/);
});
