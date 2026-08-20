/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

#[cfg(target_arch = "wasm32")]
extern crate uniffi_runtime_wasm as _;

#[uniffi::export]
/// This makes the byte array in rust, and the test in JS will compare it there.
///
/// This eliminates the possibility of two symmetrical bugs in each of the lift and
/// lower for the roundtrip tests– this just uses the Rust lower, and the Typescript
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

// --- RustBuffer-argument leak probe ------------------------------------------
//
// A counting global allocator that tracks only "big" allocations (>= 64 KiB).
// Ordinary bookkeeping (strings, small vecs) is far below that, so the only
// things this size are the RustBuffers and lifted `Vec`s produced by a large
// byte-array argument. Tests read the live count through `live_big_alloc_count`
// and assert exact equality across a loop of calls — the same counter technique
// the napi ownership tests use, chosen because RSS/heap sampling is too noisy to
// see a per-call leak of one buffer.
//
// Purpose: prove empirically whether lowering a `RustBuffer` argument leaks. If
// the runtime copies the lowered view (JSI, and napi before the adopt fix) the
// library-owned allocation is orphaned and the live count climbs by one per
// call; if it adopts or aliases (napi post-fix, wasm2) the count stays flat.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicI64, Ordering};

const BIG_THRESHOLD: usize = 64 * 1024;
static LIVE_BIG_COUNT: AtomicI64 = AtomicI64::new(0);
static LIVE_BIG_BYTES: AtomicI64 = AtomicI64::new(0);

struct CountingAlloc;

// SAFETY: delegates every allocation to the System allocator unchanged; the
// atomics only observe sizes and never alter the returned pointer.
unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() && layout.size() >= BIG_THRESHOLD {
            LIVE_BIG_COUNT.fetch_add(1, Ordering::Relaxed);
            LIVE_BIG_BYTES.fetch_add(layout.size() as i64, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.size() >= BIG_THRESHOLD {
            LIVE_BIG_COUNT.fetch_sub(1, Ordering::Relaxed);
            LIVE_BIG_BYTES.fetch_sub(layout.size() as i64, Ordering::Relaxed);
        }
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

#[uniffi::export]
/// Takes a byte array (lowered as a `RustBuffer`) and returns a scalar, so only
/// the *argument* direction allocates a big buffer. The leak probe calls this in
/// a loop; a leaked lowered argument shows up as growth in `live_big_alloc_count`.
pub fn measure_bytes(bytes: Vec<u8>) -> u32 {
    bytes.len() as u32
}

#[uniffi::export]
/// Number of live allocations >= 64 KiB the crate currently owns.
pub fn live_big_alloc_count() -> u32 {
    LIVE_BIG_COUNT.load(Ordering::Relaxed).max(0) as u32
}

#[uniffi::export]
/// Total bytes across live allocations >= 64 KiB the crate currently owns.
pub fn live_big_alloc_bytes() -> u64 {
    LIVE_BIG_BYTES.load(Ordering::Relaxed).max(0) as u64
}

// --- RustBuffer-return leak probe (callback direction) -----------------------
//
// The probe above only exercises the *argument* direction: JS lowers a buffer and
// Rust consumes it. The mirror case is a foreign callback that *returns* a buffer,
// where the foreign side lowers and Rust consumes what comes back.
//
// That direction had no coverage in any flavour, because this fixture had no
// callback interface — which is how a runtime could copy instead of adopt on the
// callback path and still pass a green suite. The counter makes it exact: if the
// foreign-lowered buffer is orphaned, the live count climbs by one per call.
#[uniffi::export(with_foreign)]
pub trait BytesProducer: Send + Sync {
    /// Returns a byte array the foreign side lowers into a `RustBuffer`. Rust owns
    /// what comes back and drops it, so a balanced implementation leaves the live
    /// count unchanged.
    fn produce(&self, size: u32) -> Vec<u8>;
}

#[uniffi::export]
/// Calls `produce` once and reports the length, dropping the returned buffer. Any
/// growth in `live_big_alloc_count` across a loop of these is a buffer the foreign
/// lowering allocated and nobody freed.
pub fn consume_produced_bytes(producer: std::sync::Arc<dyn BytesProducer>, size: u32) -> u32 {
    let bytes = producer.produce(size);
    bytes.len() as u32
}

uniffi::setup_scaffolding!();
