/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

/// Accept a string argument (measures string lowering overhead).
#[uniffi::export]
pub fn take_string(s: String) {
    let _ = s;
}

/// Return a string of the given byte length (measures string lifting overhead).
#[uniffi::export]
pub fn get_string(length: u32) -> String {
    "x".repeat(length as usize)
}

/// Accept a byte array argument (measures bytes lowering overhead).
#[uniffi::export]
pub fn take_bytes(bytes: Vec<u8>) {
    let _ = bytes;
}

/// Return a byte array of the given length (measures bytes lifting overhead).
#[uniffi::export]
pub fn get_bytes(length: u32) -> Vec<u8> {
    vec![0u8; length as usize]
}

/// Return a Vec of `repeats` copies of `s` (measures sequence + string lifting).
#[uniffi::export]
pub fn get_string_array(repeats: u32, s: String) -> Vec<String> {
    std::iter::repeat_n(s, repeats as usize).collect()
}

/// Accept a Vec of strings (measures sequence + string lowering overhead).
#[uniffi::export]
pub fn take_string_array(strings: Vec<String>) {
    let _ = strings;
}

uniffi::setup_scaffolding!();
