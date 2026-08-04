/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Smoke tests for the browser entry; every fixture goes through
// `@ubjs/wasm/node` instead.
//
// Node supplies `Response` and `instantiateStreaming`, so `openWasm` is driven
// as a page would drive it. This covers our code, not a browser engine or a
// bundler.
import { test } from "node:test";
import assert from "node:assert";
import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { openWasm } from "../../browser/src/index.js";

/** The same pre-assembled host module `module.test.ts` uses: exports
 * `memory`, `__ubrn_alloc`, `__ubrn_free`, `__ubrn_install_panic_hook` and
 * `__ubrn_emit_trampoline`, which is the minimum `UniffiNativeModule.open`
 * requires. Assembled with `wat2wasm`; see that file for the source .wat. */
const WASM = new Uint8Array([
  0, 97, 115, 109, 1, 0, 0, 0, 1, 29, 5, 96, 2, 127, 127, 0, 96, 2, 127, 127, 1,
  127, 96, 3, 127, 127, 127, 0, 96, 0, 0, 96, 4, 127, 127, 127, 127, 1, 127, 2,
  24, 1, 3, 101, 110, 118, 16, 95, 95, 117, 98, 114, 110, 95, 112, 97, 110, 105,
  99, 95, 108, 111, 103, 0, 0, 3, 5, 4, 1, 2, 3, 4, 5, 3, 1, 0, 1, 7, 92, 5, 6,
  109, 101, 109, 111, 114, 121, 2, 0, 12, 95, 95, 117, 98, 114, 110, 95, 97,
  108, 108, 111, 99, 0, 1, 11, 95, 95, 117, 98, 114, 110, 95, 102, 114, 101,
  101, 0, 2, 25, 95, 95, 117, 98, 114, 110, 95, 105, 110, 115, 116, 97, 108,
  108, 95, 112, 97, 110, 105, 99, 95, 104, 111, 111, 107, 0, 3, 22, 95, 95, 117,
  98, 114, 110, 95, 101, 109, 105, 116, 95, 116, 114, 97, 109, 112, 111, 108,
  105, 110, 101, 0, 4, 10, 18, 4, 5, 0, 65, 128, 8, 11, 2, 0, 11, 2, 0, 11, 4,
  0, 65, 0, 11,
]);

function wasmResponse(bytes: Uint8Array): Response {
  return new Response(bytes as unknown as BodyInit, {
    headers: { "content-type": "application/wasm" },
  });
}

test("browser openWasm accepts a Response", async () => {
  const mod = await openWasm(wasmResponse(WASM));
  assert.ok(mod, "expected a module back");
});

test("browser openWasm accepts a Promise<Response>", async () => {
  // Its own WasmSource union advertises this; a bare `fetch(...)` produces it.
  const mod = await openWasm(Promise.resolve(wasmResponse(WASM)));
  assert.ok(mod, "expected a module back from a Promise<Response>");
});

test("browser openWasm accepts raw bytes", async () => {
  const mod = await openWasm(WASM);
  assert.ok(mod, "expected a module back from a BufferSource");
});

test("the browser entry pulls in no node built-ins", async () => {
  // A generated `-ffi.ts` importing `@ubjs/wasm/node` would drag
  // `node:fs/promises` and `node:url` into every browser and React Native
  // bundle.
  const src = await readFile(
    fileURLToPath(new URL("../../browser/src/index.ts", import.meta.url)),
    "utf8",
  );
  const offenders = [...src.matchAll(/from\s+"(node:[^"]+)"/g)].map(
    (m) => m[1],
  );
  assert.deepStrictEqual(
    offenders,
    [],
    `browser entry must not import node built-ins, found: ${offenders.join(", ")}`,
  );
});
