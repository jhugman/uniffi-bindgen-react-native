/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! State that outlives a JS runtime.
//!
//! The test-runner evaluates each script against two runtimes in one process
//! and never unloads this library between them, which is the shape of a React
//! Native full reload. Everything here is process-global on purpose: it is the
//! only way a test can hold something built by the first runtime and reach it
//! from the second.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

uniffi::setup_scaffolding!();

static ITERATIONS: AtomicU32 = AtomicU32::new(0);

/// How many runtimes have run the test script in this process, counting this
/// one. Each runtime gets a fresh module graph, so Rust state is the only thing
/// that can tell the two apart.
#[uniffi::export]
pub fn note_iteration() -> u32 {
    ITERATIONS.fetch_add(1, Ordering::Relaxed) + 1
}

#[uniffi::export(with_foreign)]
pub trait Greeter: Send + Sync {
    fn greet(&self) -> String;
}

static STASHED: Mutex<Option<Arc<dyn Greeter>>> = Mutex::new(None);

/// Keep `greeter` alive past the runtime that created it.
///
/// Whatever was stashed before is leaked rather than dropped. Dropping it calls
/// the foreign `uniffi_free` for a handle minted by a runtime that no longer
/// exists, and each runtime's handle map numbers from 1 again, so that free
/// lands on a live handle belonging to the runtime doing the dropping.
#[uniffi::export]
pub fn stash_greeter(greeter: Arc<dyn Greeter>) {
    let previous = STASHED.lock().expect("stash poisoned").replace(greeter);
    std::mem::forget(previous);
}

/// Call whatever is stashed, or `None` if nothing is.
#[uniffi::export]
pub fn invoke_stashed_greeter() -> Option<String> {
    let greeter = STASHED.lock().expect("stash poisoned").clone();
    greeter.map(|g| g.greet())
}

static PINGER_STARTED: AtomicBool = AtomicBool::new(false);
static PINGER_RETURNED: AtomicBool = AtomicBool::new(false);

/// Ping `greeter` from a Rust-owned thread until it stops answering.
///
/// Every call crosses threads, so each one is posted onto the JS thread and the
/// worker blocks until it runs. A runtime destroyed mid-rendezvous is what
/// strands the worker; a disarmed trampoline answers with an empty string,
/// which is the signal to stop.
#[uniffi::export]
pub fn start_foreign_thread_pinger(greeter: Arc<dyn Greeter>) {
    PINGER_STARTED.store(true, Ordering::Release);
    std::thread::spawn(move || {
        while greeter.greet() == "pong" {}
        PINGER_RETURNED.store(true, Ordering::Release);
    });
}

#[uniffi::export]
pub fn pinger_started() -> bool {
    PINGER_STARTED.load(Ordering::Acquire)
}

/// True once the pinger's thread has left its loop — the proof that a parked
/// rendezvous was released rather than waited on forever.
#[uniffi::export]
pub fn pinger_returned() -> bool {
    PINGER_RETURNED.load(Ordering::Acquire)
}
