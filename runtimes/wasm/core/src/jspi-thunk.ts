/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

/** The lowered WASM signature, after adding sret/status pointers. */
export type WasmScalar = "i32" | "i64" | "f32" | "f64";

const types = { i32: 0x7f, i64: 0x7e, f32: 0x7d, f64: 0x7c };
const loads = { i32: 0x28, i64: 0x29, f32: 0x2a, f64: 0x2b };
const stores = { i32: 0x36, i64: 0x37, f32: 0x38, f64: 0x39 };
const alignment = { i32: 2, i64: 3, f32: 2, f64: 3 };

function unsigned(n: number): number[] {
  const bytes: number[] = [];
  do {
    const byte = n & 0x7f;
    n >>>= 7;
    bytes.push(byte | (n ? 0x80 : 0));
  } while (n);
  return bytes;
}

function name(s: string): number[] {
  const bytes = new TextEncoder().encode(s);
  return [...unsigned(bytes.length), ...bytes];
}

function section(id: number, bytes: number[]): number[] {
  return [id, ...unsigned(bytes.length), ...bytes];
}

/**
 * Emit `(frame: i32) -> void`, importing a raw WASM function and its memory.
 * Arguments occupy 8-byte slots in frame order; a scalar result occupies the
 * next slot. Pointers (including sret/status) are ordinary i32 arguments.
 *
 * Install the exported `call` in the Rust module's function table and invoke
 * it through an instrumented JSPI entry. Import `target` as the raw WASM export,
 * never a JS wrapper: suspension must not cross an intervening JS frame.
 * This thunk itself allocates no shadow stack and needs no instrumentation.
 * Its memory must be the same unshared wasm32 memory used by the target.
 *
 * This encoder is internal to the wasm2 JSPI dispatcher.
 */
export function emitJspiThunk(
  args: WasmScalar[],
  ret?: WasmScalar,
): Uint8Array {
  for (const type of [...args, ...(ret ? [ret] : [])]) {
    if (!Object.hasOwn(types, type))
      throw new Error(`Invalid WASM scalar ${type}`);
  }
  // The frame offsets and encoded vector lengths must fit wasm32.
  if (args.length > 0x1ffffffe)
    throw new Error("JSPI argument frame exceeds wasm32");
  const signature = [
    0x60,
    ...unsigned(args.length),
    ...args.map((t) => types[t]),
    ...(ret ? [1, types[ret]] : [0]),
  ];
  const typeSection = section(1, [2, ...signature, 0x60, 1, types.i32, 0]);
  const imports = section(2, [
    2,
    ...name("env"),
    ...name("target"),
    0,
    0, // function, type 0
    ...name("env"),
    ...name("memory"),
    2,
    0,
    0, // memory, min 0, no max
  ]);
  const functions = section(3, [1, 1]); // one local function, type 1
  const exports = section(7, [1, ...name("call"), 0, 1]);
  const body = [0]; // no local declarations
  if (ret) body.push(0x20, 0); // result store's base pointer, under call args
  args.forEach((type, i) => {
    body.push(0x20, 0, loads[type], alignment[type], ...unsigned(i * 8));
  });
  body.push(0x10, 0); // call imported WASM target
  if (ret) body.push(stores[ret], alignment[ret], ...unsigned(args.length * 8));
  body.push(0x0b);
  return new Uint8Array([
    0,
    0x61,
    0x73,
    0x6d,
    1,
    0,
    0,
    0,
    ...typeSection,
    ...imports,
    ...functions,
    ...exports,
    ...section(10, [1, ...unsigned(body.length), ...body]),
  ]);
}
