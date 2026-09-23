/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

/// The single function exercised by the JSI2 walking skeleton: two scalar
/// arguments, a scalar return, and (implicitly) a RustCallStatus out-param.
#[uniffi::export]
pub fn add(a: u32, b: u32) -> u32 {
    a.wrapping_add(b)
}

/// Returns a short string — exercises a RustBuffer return over the C ABI.
#[uniffi::export]
pub fn describe(n: u32) -> String {
    format!("n={n}")
}

uniffi::setup_scaffolding!();
