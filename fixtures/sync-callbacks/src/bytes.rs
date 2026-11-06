/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// --- RustBuffer-return leak probe (callback direction) -----------------------
//
// A foreign callback that *returns* a buffer: the foreign side lowers and Rust
// consumes what comes back. If the runtime copies instead of adopting, the
// foreign-lowered buffer is orphaned and the live count climbs by one per call.
//
// A counting global allocator tracks only "big" allocations (>= 64 KiB), far
// above ordinary bookkeeping. Tests assert exact equality of the live count
// across a loop of calls, because RSS/heap sampling is too noisy to see a
// per-call leak of one buffer. A global allocator is per cdylib, so this crate
// carries its own copy.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

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
/// Number of live allocations >= 64 KiB the crate currently owns.
pub fn live_big_alloc_count() -> u32 {
    LIVE_BIG_COUNT.load(Ordering::Relaxed).max(0) as u32
}

#[uniffi::export]
/// Total bytes across live allocations >= 64 KiB the crate currently owns.
pub fn live_big_alloc_bytes() -> u64 {
    LIVE_BIG_BYTES.load(Ordering::Relaxed).max(0) as u64
}

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
pub fn consume_produced_bytes(producer: Arc<dyn BytesProducer>, size: u32) -> u32 {
    let bytes = producer.produce(size);
    bytes.len() as u32
}

// --- RustBuffer-argument leak probe (callback direction) ---------------------
//
// Rust lowers a buffer and passes it as an *argument* to a foreign callback. The
// foreign side owns that buffer and must free it once it has lifted it, or the
// live count climbs by one per call.
#[uniffi::export(with_foreign)]
pub trait BytesConsumer: Send + Sync {
    /// Receives a byte array that Rust lowered into a `RustBuffer`.
    fn consume(&self, bytes: Vec<u8>) -> u32;
}

#[uniffi::export]
/// Passes a `size`-byte array to `consume` and returns what it reports. Any growth
/// in `live_big_alloc_count` across a loop of these is a callback argument the
/// foreign side never freed.
pub fn send_bytes_to_consumer(consumer: Arc<dyn BytesConsumer>, size: u32) -> u32 {
    consumer.consume(vec![0; size as usize])
}
