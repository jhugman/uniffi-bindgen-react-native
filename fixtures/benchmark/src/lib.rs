/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

#[cfg(target_arch = "wasm32")]
extern crate uniffi_runtime_wasm as _;

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// String / bytes / array roundtrips (original suite)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// FFI overhead micro suite
// ---------------------------------------------------------------------------

/// Zero-payload call — isolates pure FFI crossing cost.
#[uniffi::export]
pub fn noop() {}

/// Two-scalar marshaling — no buffer involved.
#[uniffi::export]
pub fn add_u32(a: u32, b: u32) -> u32 {
    a.wrapping_add(b)
}

/// Object handle + method dispatch — measures handle passing and vtable cost.
#[derive(uniffi::Object)]
pub struct Counter(AtomicU32);

#[uniffi::export]
impl Counter {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self(AtomicU32::new(0)))
    }

    pub fn increment(&self) -> u32 {
        self.0.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }
}

/// Constructors are `new` on sync flavors and `create()` under async
/// delivery; a factory function lets one script build a Counter on both.
#[uniffi::export]
pub fn make_counter() -> Arc<Counter> {
    Counter::new()
}

const BURN_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// Deterministic compute loop; the checksum keeps the optimiser honest.
/// wasm32 has no clock, so the script calibrates `iterations` to a wall time.
#[uniffi::export]
pub fn burn(iterations: u64) -> u64 {
    let mut x = BURN_SEED;
    for i in 0..iterations {
        x ^= i;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 31;
    }
    x
}

// ---------------------------------------------------------------------------
// Wide record — many fields, mixed types
// ---------------------------------------------------------------------------

#[derive(uniffi::Record, Clone)]
pub struct LargeRecord {
    pub a_u8: u8,
    pub a_i8: i8,
    pub a_u16: u16,
    pub a_i16: i16,
    pub a_u32: u32,
    pub a_i32: i32,
    pub a_u64: u64,
    pub a_i64: i64,
    pub a_f32: f32,
    pub a_f64: f64,
    pub a_bool: bool,
    pub s1: String,
    pub s2: String,
    pub s3: String,
    pub s4: String,
    pub opt_str: Option<String>,
    pub opt_u32: Option<u32>,
    pub bytes: Vec<u8>,
    pub strs: Vec<String>,
    pub ints: Vec<u32>,
}

#[uniffi::export]
pub fn get_large_record() -> LargeRecord {
    LargeRecord {
        a_u8: 1,
        a_i8: -1,
        a_u16: 2,
        a_i16: -2,
        a_u32: 3,
        a_i32: -3,
        a_u64: 4,
        a_i64: -4,
        a_f32: 5.5,
        a_f64: 6.5,
        a_bool: true,
        s1: "alpha".to_string(),
        s2: "beta".to_string(),
        s3: "gamma".to_string(),
        s4: "delta".to_string(),
        opt_str: Some("optional".to_string()),
        opt_u32: Some(7),
        bytes: vec![0u8; 32],
        strs: vec!["one".to_string(), "two".to_string(), "three".to_string()],
        ints: vec![10, 20, 30, 40, 50],
    }
}

#[uniffi::export]
pub fn take_large_record(r: LargeRecord) {
    let _ = r;
}

// ---------------------------------------------------------------------------
// Recursive enum — depth-controlled binary tree
// ---------------------------------------------------------------------------

/// uniffi 0.31's `#[derive(uniffi::Enum)]` does not implement `Lift`/`Lower`
/// for `Box<Self>`, so we model the recursion via `Vec<Tree>` — heap-backed
/// and equivalent for marshaling-cost measurement. `build_tree` always
/// produces exactly two children, mimicking a binary tree.
#[derive(uniffi::Enum)]
pub enum Tree {
    Leaf,
    Node { children: Vec<Tree> },
}

#[uniffi::export]
pub fn build_tree(depth: u32) -> Tree {
    if depth == 0 {
        Tree::Leaf
    } else {
        Tree::Node {
            children: vec![build_tree(depth - 1), build_tree(depth - 1)],
        }
    }
}

#[uniffi::export]
pub fn count_leaves(t: Tree) -> u32 {
    fn walk(t: &Tree) -> u32 {
        match t {
            Tree::Leaf => 1,
            Tree::Node { children } => children.iter().map(walk).sum(),
        }
    }
    walk(&t)
}

// ---------------------------------------------------------------------------
// Async — immediately-ready variants
// ---------------------------------------------------------------------------
//
// These futures resolve in a single poll. They isolate the async FFI overhead
// (future handle, waker registration, completion plumbing) from any actual
// scheduling latency. Compare to the sync versions to see what `async` costs
// at the boundary when the work itself is free.

#[uniffi::export]
pub async fn noop_async() {}

#[uniffi::export]
pub async fn add_u32_async(a: u32, b: u32) -> u32 {
    a.wrapping_add(b)
}

#[uniffi::export]
pub async fn get_string_async(length: u32) -> String {
    "x".repeat(length as usize)
}

#[uniffi::export]
pub async fn take_string_async(s: String) {
    let _ = s;
}

#[uniffi::export]
pub async fn get_bytes_async(length: u32) -> Vec<u8> {
    vec![0u8; length as usize]
}

#[uniffi::export]
pub async fn take_bytes_async(bytes: Vec<u8>) {
    let _ = bytes;
}

#[uniffi::export]
pub async fn get_string_array_async(repeats: u32, s: String) -> Vec<String> {
    std::iter::repeat_n(s, repeats as usize).collect()
}

#[uniffi::export]
pub async fn take_string_array_async(strings: Vec<String>) {
    let _ = strings;
}

// ---------------------------------------------------------------------------
// Callback round trips
// ---------------------------------------------------------------------------

/// Async-only: async delivery forbids sync callback methods, and the sync
/// flavors can drive an async callback too.
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait Ping: Send + Sync {
    async fn ping(&self, n: u32) -> u32;
}

/// Calls `cb.ping` `times` times in sequence and returns the sum.
#[uniffi::export]
pub async fn call_ping(cb: Arc<dyn Ping>, times: u32) -> u32 {
    let mut sum = 0u32;
    for i in 0..times {
        sum = sum.wrapping_add(cb.ping(i).await);
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burn_is_deterministic_and_depends_on_count() {
        assert_eq!(burn(0), BURN_SEED);
        assert_eq!(burn(1000), burn(1000));
        assert_ne!(burn(1000), burn(1001));
    }

    #[test]
    fn make_counter_starts_at_zero() {
        assert_eq!(make_counter().increment(), 1);
    }
}

uniffi::setup_scaffolding!();
