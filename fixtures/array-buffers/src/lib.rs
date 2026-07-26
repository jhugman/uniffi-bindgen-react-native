/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// Sibling to the `strict-byte-arrays` fixture: that one forces `Vec<u8>` to
// be emitted as `Uint8Array` (`strictByteArrays = true`); this one leaves the
// default TypeScript config in place, so `Vec<u8>` is emitted as the plain
// `ArrayBuffer` that most fixtures (and the FfiConverterArrayBuffer cursor
// path) use.

#[uniffi::export]
/// This makes the byte array in rust, and the test in JS will compare it there.
///
/// This eliminates the possibility of two symmetrical bugs in each of the lift and
/// lower for the roundtrip tests– this just uses the Rust lower, and the Typescript
/// lift.
pub fn well_known_bytes() -> Vec<u8> {
    vec![1, 2, 3, 255]
}

#[uniffi::export]
/// This uses a byte array to pass an argument and return, so it uses lift/lower methods.
pub fn identity_bytes(bytes: Vec<u8>) -> Vec<u8> {
    bytes
}

#[uniffi::export]
/// This uses an option to force the lift/lower machinery to use read and write
/// directly from the Option lift and lower, not from the byte array lift and lower.
pub fn identity_bytes_forced_read(bytes: Option<Vec<u8>>) -> Option<Vec<u8>> {
    bytes
}

uniffi::setup_scaffolding!();
