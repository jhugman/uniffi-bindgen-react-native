/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Hand-rolled wasm encoder for the *single* shape we emit: a trampoline
// function that pushes a shape id and forwards its args to a JS-imported
// `env.__ubrn_dispatch`. Encoding here rather than in the helper crate keeps
// the wasm-encoder crate — some 4 MB per cdylib — out of every fixture.
//
// Output layout:
//
//   (module
//     (type $trampoline (func (param ...) (result ...)))
//     (type $dispatch   (func (param i32 ...) (result ...)))
//     (import "env" "__ubrn_dispatch" (func $dispatch (type 1)))
//     (func $trampoline (type 0)
//       i32.const <shape_id>
//       local.get 0 ... local.get N
//       call $dispatch)
//     (export "trampoline" (func $trampoline)))

export const TAG_I32 = 0;
export const TAG_I64 = 1;
export const TAG_F32 = 2;
export const TAG_F64 = 3;

const VALTYPE_I32 = 0x7f;
const VALTYPE_I64 = 0x7e;
const VALTYPE_F32 = 0x7d;
const VALTYPE_F64 = 0x7c;

function tagToValtype(tag: number): number {
  switch (tag) {
    case TAG_I32:
      return VALTYPE_I32;
    case TAG_I64:
      return VALTYPE_I64;
    case TAG_F32:
      return VALTYPE_F32;
    case TAG_F64:
      return VALTYPE_F64;
    default:
      throw new Error(`emitTrampoline: unknown type tag ${tag}`);
  }
}

/** Append unsigned LEB128 of `n` (≥ 0) to `out`. */
function leb128(out: number[], n: number): void {
  do {
    let byte = n & 0x7f;
    n >>>= 7;
    if (n !== 0) byte |= 0x80;
    out.push(byte);
  } while (n !== 0);
}

/** Append signed LEB128 of `n` to `out`. */
function sleb128(out: number[], n: number): void {
  for (;;) {
    let byte = n & 0x7f;
    // Arithmetic shift right preserves sign.
    n >>= 7;
    const sign = byte & 0x40;
    if ((n === 0 && sign === 0) || (n === -1 && sign !== 0)) {
      out.push(byte);
      return;
    }
    out.push(byte | 0x80);
  }
}

const ASCII = new TextEncoder();
function encodeName(out: number[], s: string): void {
  const bytes = ASCII.encode(s);
  leb128(out, bytes.length);
  for (const b of bytes) out.push(b);
}

function pushSection(out: number[], id: number, payload: number[]): void {
  out.push(id);
  leb128(out, payload.length);
  for (const b of payload) out.push(b);
}

function encodeFuncType(
  out: number[],
  params: number[],
  results: number[],
): void {
  out.push(0x60);
  leb128(out, params.length);
  for (const p of params) out.push(p);
  leb128(out, results.length);
  for (const r of results) out.push(r);
}

/**
 * Emit a wasm trampoline module. `paramTags` are the trampoline's
 * exposed parameter type tags; `returnTag` is `undefined` for void.
 * `shapeId` is forwarded as the first arg to `__ubrn_dispatch`. */
export function emitTrampoline(
  shapeId: number,
  paramTags: number[],
  returnTag: number | undefined,
): Uint8Array {
  const params = paramTags.map(tagToValtype);
  const results = returnTag !== undefined ? [tagToValtype(returnTag)] : [];

  // -- Type section: type 0 = trampoline, type 1 = dispatch (i32, ...params)
  const typeSection: number[] = [];
  leb128(typeSection, 2); // 2 type definitions
  encodeFuncType(typeSection, params, results);
  encodeFuncType(typeSection, [VALTYPE_I32, ...params], results);

  // -- Import section: env.__ubrn_dispatch (func type 1) — function index 0
  const importSection: number[] = [];
  leb128(importSection, 1);
  encodeName(importSection, "env");
  encodeName(importSection, "__ubrn_dispatch");
  importSection.push(0x00); // import kind: function
  leb128(importSection, 1); // type index 1

  // -- Function section: 1 local function of type 0 (function index 1)
  const funcSection: number[] = [];
  leb128(funcSection, 1);
  leb128(funcSection, 0);

  // -- Export section: "trampoline" -> function index 1
  const exportSection: number[] = [];
  leb128(exportSection, 1);
  encodeName(exportSection, "trampoline");
  exportSection.push(0x00); // export kind: function
  leb128(exportSection, 1); // function index 1

  // -- Code section: one function body, no locals.
  //    Body: i32.const shapeId, local.get 0..N-1, call 0, end.
  const body: number[] = [];
  leb128(body, 0); // 0 local declarations
  body.push(0x41); // i32.const
  sleb128(body, shapeId);
  for (let i = 0; i < params.length; i++) {
    body.push(0x20); // local.get
    leb128(body, i);
  }
  body.push(0x10); // call
  leb128(body, 0); // function index 0 (the imported dispatch)
  body.push(0x0b); // end

  const codeSection: number[] = [];
  leb128(codeSection, 1); // 1 function body
  leb128(codeSection, body.length); // body length
  for (const b of body) codeSection.push(b);

  const out: number[] = [
    0x00,
    0x61,
    0x73,
    0x6d, // magic: \0asm
    0x01,
    0x00,
    0x00,
    0x00, // version: 1
  ];
  pushSection(out, 1, typeSection);
  pushSection(out, 2, importSection);
  pushSection(out, 3, funcSection);
  pushSection(out, 7, exportSection);
  pushSection(out, 10, codeSection);

  return new Uint8Array(out);
}
