/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Exercises what only an asynchronous player can get wrong: object
//! handles cloned fire-and-forget, callbacks crossing a port, futures
//! cancelled mid-flight, and buffers transferred rather than copied.

#[cfg(target_arch = "wasm32")]
extern crate uniffi_runtime_wasm as _;

use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use ubrn_testing::timer::{TimerFuture, TimerService};

uniffi::setup_scaffolding!();

static LIVE: AtomicI32 = AtomicI32::new(0);

/// Live `Counter` objects: every construction adds one, every drop removes one.
#[uniffi::export]
pub fn live_counters() -> i32 {
    LIVE.load(Ordering::SeqCst)
}

#[derive(uniffi::Object)]
pub struct Counter {
    value: Mutex<i64>,
}

impl Drop for Counter {
    fn drop(&mut self) {
        LIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

#[uniffi::export]
impl Counter {
    #[uniffi::constructor]
    pub fn new(start: i64) -> Arc<Self> {
        LIVE.fetch_add(1, Ordering::SeqCst);
        Arc::new(Self {
            value: Mutex::new(start),
        })
    }

    pub fn value(&self) -> i64 {
        *self.value.lock().unwrap()
    }

    pub fn add(&self, n: i64) -> i64 {
        let mut v = self.value.lock().unwrap();
        *v += n;
        *v
    }

    /// Sums this counter with two more. Passing the same object three times
    /// makes one call carry three clones of one handle.
    pub fn sum_with(&self, a: Arc<Counter>, b: Arc<Counter>) -> i64 {
        self.value() + a.value() + b.value()
    }
}

#[derive(uniffi::Record)]
pub struct Ledger {
    pub name: String,
    pub counters: Vec<Arc<Counter>>,
    pub primary: Option<Arc<Counter>>,
}

/// Lowers a record holding objects (list and optional) and lifts one back.
#[uniffi::export]
pub fn total(ledger: Ledger) -> i64 {
    ledger.counters.iter().map(|c| c.value()).sum::<i64>()
        + ledger.primary.map(|c| c.value()).unwrap_or(0)
}

#[uniffi::export]
pub fn make_ledger(name: String, values: Vec<i64>) -> Ledger {
    let counters: Vec<_> = values.into_iter().map(Counter::new).collect();
    let primary = counters.first().cloned();
    Ledger {
        name,
        counters,
        primary,
    }
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ChannelError {
    #[error("flat: {0}")]
    Flat(String),
}

#[uniffi::export]
pub fn fail_flat(msg: String) -> Result<(), ChannelError> {
    Err(ChannelError::Flat(msg))
}

#[derive(Debug, thiserror::Error, uniffi::Object)]
#[uniffi::export(Debug)]
#[error("rich error {code}")]
pub struct RichError {
    code: i32,
}

#[uniffi::export]
impl RichError {
    pub fn code(&self) -> i32 {
        self.code
    }
}

#[uniffi::export]
pub fn fail_rich(code: i32) -> Result<(), Arc<RichError>> {
    Err(Arc::new(RichError { code }))
}

/// Round-trips bytes untouched and reports a checksum, so a transferred
/// buffer that was detached or truncated shows up as a length or sum change.
#[uniffi::export]
pub fn echo_bytes(bytes: Vec<u8>) -> Vec<u8> {
    bytes
}

#[uniffi::export]
pub fn checksum(bytes: Vec<u8>) -> u64 {
    bytes.iter().map(|b| *b as u64).sum()
}

#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait Greeter: Send + Sync {
    async fn greet(&self, name: String) -> String;
}

#[uniffi::export]
pub async fn greet_via(greeter: Arc<dyn Greeter>, name: String) -> String {
    greeter.greet(name).await
}

/// Resolves after `ms`.
#[uniffi::export]
pub async fn sleep_ms(ms: u16) -> u16 {
    TimerFuture::sleep(Duration::from_millis(ms.into())).await;
    ms
}

/// Sleeps for a long time; the test cancels it.
#[uniffi::export]
pub async fn sleep_forever() -> bool {
    TimerFuture::sleep(Duration::from_secs(60)).await;
    true
}
