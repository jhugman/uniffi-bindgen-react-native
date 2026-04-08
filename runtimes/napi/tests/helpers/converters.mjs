/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// UniFFI wire-format serialization helpers.
//
// In UniFFI 0.31, top-level function String arguments and return values are
// stored as raw UTF-8 bytes in a RustBuffer (no length prefix). Strings that
// appear as *fields* inside compound types (records, enums) continue to use
// the older 4-byte big-endian i32 length prefix + UTF-8 bytes format.

const encoder = new TextEncoder();
const decoder = new TextDecoder();

/**
 * Lower a JS string into a Uint8Array for use as a top-level UniFFI function
 * argument (uniffi 0.31+): raw UTF-8 bytes, no length prefix.
 */
export function lowerString(s) {
  return encoder.encode(s);
}

/**
 * Lift a Uint8Array (from a top-level UniFFI function return) into a JS string
 * (uniffi 0.31+): raw UTF-8 bytes, no length prefix.
 */
export function liftString(buf) {
  return decoder.decode(buf);
}

/**
 * Read a length-prefixed string field from a DataView at the given byte offset.
 * Used for String fields inside compound types (records, enums) which are
 * serialized as: 4-byte big-endian i32 length, then UTF-8 bytes.
 * Returns { value: string, nextOffset: number }.
 */
function readStringField(view, offset) {
  const len = view.getInt32(offset, false);
  const bytes = new Uint8Array(view.buffer, view.byteOffset + offset + 4, len);
  return { value: decoder.decode(bytes), nextOffset: offset + 4 + len };
}

/**
 * Lift a UniFFI error enum from a Uint8Array (from RustCallStatus.errorBuf).
 * Reads: 4-byte big-endian Int32 variant index, then variant fields.
 * Returns { variant: number, ...fields }.
 *
 * For ArithmeticError::DivisionByZero { reason: String }:
 *   variant=1, reason=<string field with 4-byte length prefix>
 */
export function liftArithmeticError(buf) {
  const view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  const variant = view.getInt32(0, false);
  const { value: reason } = readStringField(view, 4);
  return { variant, reason };
}
