// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
// Retain the player's alloc/free/panic-hook exports.
extern crate uniffi_runtime_wasm;
uniffi_runtime_wasm::export_jspi_entry!();

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(suspending, catch, js_namespace = globalThis, js_name = jspiFixtureRequest)]
    fn request(id: u32) -> Result<u32, JsValue>;
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FixtureError {
    #[error("synthetic rejection")]
    Rejected,
}

#[uniffi::export]
pub fn compute(id: u32, rounds: u32) -> Result<Vec<u8>, FixtureError> {
    let mut stack = [id; 256];
    let mut sum = 0;
    for _ in 0..rounds {
        sum += request(id).map_err(|_| FixtureError::Rejected)?;
        assert!(std::hint::black_box(&mut stack).iter().all(|v| *v == id));
    }
    Ok(format!("{id}:{sum}").into_bytes())
}

#[uniffi::export]
pub fn synchronous(value: u32) -> u32 {
    value + 1
}

#[uniffi::export]
pub fn mixed(id: u32, value: i64, factor: f32, offset: f64) -> Result<f64, FixtureError> {
    let result = value as f64 * factor as f64 + offset;
    request(id).map_err(|_| FixtureError::Rejected)?;
    Ok(std::hint::black_box(result))
}

#[uniffi::export]
pub fn wide(id: u32, value: u64) -> Result<u64, FixtureError> {
    request(id).map_err(|_| FixtureError::Rejected)?;
    Ok(value)
}

#[uniffi::export]
pub fn narrow(id: u32, value: f32) -> Result<f32, FixtureError> {
    request(id).map_err(|_| FixtureError::Rejected)?;
    Ok(value)
}

#[uniffi::export]
pub fn scalar(id: u32, value: i32) -> Result<i32, FixtureError> {
    request(id).map_err(|_| FixtureError::Rejected)?;
    Ok(value)
}

#[uniffi::export]
pub fn complete_void(id: u32) -> Result<(), FixtureError> {
    request(id).map_err(|_| FixtureError::Rejected)?;
    Ok(())
}

#[uniffi::export]
pub fn bytes(id: u32, value: Vec<u8>) -> Result<Vec<u8>, FixtureError> {
    request(id).map_err(|_| FixtureError::Rejected)?;
    Ok(value)
}

#[uniffi::export]
pub fn combine(id: u32, mut left: Vec<u8>, right: Vec<u8>) -> Result<Vec<u8>, FixtureError> {
    request(id).map_err(|_| FixtureError::Rejected)?;
    left.extend(right);
    Ok(left)
}

#[uniffi::export]
pub fn text(id: u32, value: String) -> Result<String, FixtureError> {
    request(id).map_err(|_| FixtureError::Rejected)?;
    Ok(value)
}

#[derive(Clone, uniffi::Record)]
pub struct Payload {
    pub label: String,
    pub value: u32,
}

#[uniffi::export]
pub fn record(id: u32, value: Payload) -> Result<Payload, FixtureError> {
    request(id).map_err(|_| FixtureError::Rejected)?;
    Ok(value)
}

#[uniffi::export]
pub fn no_arguments() -> u32 {
    request(99).unwrap_or(0)
}

#[uniffi::export]
pub async fn ordinary_future(value: u32) -> u32 {
    value + 1
}

uniffi::setup_scaffolding!();

// Track suspended futures and deterministic cleanup.
use std::{cell::RefCell, collections::HashMap, future::poll_fn, task::Poll};
thread_local! {
    static WAKERS: RefCell<HashMap<u32, std::task::Waker>> = RefCell::default();
    static FUTURE_DROPS: RefCell<u32> = const { RefCell::new(0) };
}
struct FutureGuard(u32);
impl Drop for FutureGuard {
    fn drop(&mut self) {
        WAKERS.with_borrow_mut(|w| w.remove(&self.0));
        FUTURE_DROPS.with_borrow_mut(|n| *n += 1);
    }
}
#[uniffi::export]
pub async fn suspended_future(id: u32, pending_after: bool) -> Result<Vec<u8>, FixtureError> {
    let _guard = FutureGuard(id);
    let mut stack = [id; 256];
    let value = request(id).map_err(|_| FixtureError::Rejected)?;
    assert!(std::hint::black_box(&mut stack).iter().all(|v| *v == id));
    if pending_after {
        let mut first = true;
        poll_fn(|cx| {
            if first {
                first = false;
                WAKERS.with_borrow_mut(|w| w.insert(id, cx.waker().clone()));
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
    }
    Ok(vec![value as u8; 1024 * 1024])
}
fn wake_future(id: u32) {
    WAKERS
        .with_borrow_mut(|w| w.remove(&id))
        .expect("missing waker")
        .wake();
}
fn future_drops() -> u32 {
    FUTURE_DROPS.with_borrow(|n| *n)
}

#[uniffi::export]
pub fn wake_selected_future(id: u32) {
    wake_future(id);
}
#[uniffi::export]
pub fn selected_future_drops() -> u32 {
    future_drops()
}
#[uniffi::export]
pub async fn async_combine(
    id: u32,
    mut left: Vec<u8>,
    right: Vec<u8>,
) -> Result<Vec<u8>, FixtureError> {
    request(id).map_err(|_| FixtureError::Rejected)?;
    left.extend(right);
    Ok(left)
}
#[uniffi::export]
pub async fn async_scalar(id: u32) -> Result<u32, FixtureError> {
    request(id).map_err(|_| FixtureError::Rejected)
}
#[uniffi::export]
pub async fn async_void(id: u32) -> Result<(), FixtureError> {
    request(id).map_err(|_| FixtureError::Rejected)?;
    Ok(())
}
#[uniffi::export]
pub async fn ordinary_bytes() -> Vec<u8> {
    vec![42]
}

fn suspend(id: u32) -> Result<u32, FixtureError> {
    request(id).map_err(|_| FixtureError::Rejected)
}
#[uniffi::export]
impl Payload {
    pub fn append(&self, id: u32, suffix: String) -> Result<Self, FixtureError> {
        suspend(id)?;
        Ok(Self {
            label: format!("{}{}", self.label, suffix),
            value: self.value,
        })
    }

    pub async fn async_copy(&self) -> Result<Self, FixtureError> {
        suspend(self.value)?;
        Ok(self.clone())
    }

    pub fn delayed(&self, id: u32) -> Result<Self, FixtureError> {
        let mut result = self.clone();
        result.value += suspend(id)?;
        Ok(result)
    }

    pub fn touch(&self, id: u32) -> Result<(), FixtureError> {
        suspend(id)?;
        Ok(())
    }

    pub fn plain_touch(&self, id: u32) {
        suspend(id).expect("synthetic success");
    }
}

#[derive(Clone, uniffi::Enum)]
pub enum Mode {
    First,
    Second,
}

#[uniffi::export]
impl Mode {
    pub fn delayed(&self, id: u32) -> Result<Self, FixtureError> {
        suspend(id)?;
        Ok(self.clone())
    }
}

#[derive(Clone, uniffi::Enum)]
pub enum Choice {
    Label { value: String },
}

#[uniffi::export]
impl Choice {
    pub async fn async_copy(&self) -> Result<Self, FixtureError> {
        self.delayed(57)
    }

    pub async fn async_delayed(&self, id: u32) -> Result<Self, FixtureError> {
        self.delayed(id)
    }

    pub fn delayed(&self, id: u32) -> Result<Self, FixtureError> {
        suspend(id)?;
        Ok(self.clone())
    }
}

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Arc,
};
static DROPS: AtomicU32 = AtomicU32::new(0);

#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct Processor {
    id: u32,
    label: String,
}

#[uniffi::export]
impl Processor {
    #[uniffi::constructor]
    pub fn new(id: u32, label: String) -> Result<Arc<Self>, FixtureError> {
        let result = Arc::new(Self { id, label });
        suspend(id)?;
        Ok(result)
    }

    #[uniffi::constructor]
    pub fn from_label(id: u32, label: String) -> Result<Arc<Self>, FixtureError> {
        Self::new(id, label)
    }

    #[uniffi::constructor]
    pub async fn async_new(id: u32, label: String) -> Result<Arc<Self>, FixtureError> {
        Self::new(id, label)
    }

    pub async fn async_work(&self, id: u32) -> Result<String, FixtureError> {
        self.work(id)
    }

    pub fn work(&self, id: u32) -> Result<String, FixtureError> {
        let value = suspend(id)?;
        Ok(format!("{}:{value}", self.label))
    }

    pub fn touch(&self, id: u32) -> Result<(), FixtureError> {
        suspend(id)?;
        Ok(())
    }

    pub fn join(&self, other: Arc<Self>, id: u32) -> Result<String, FixtureError> {
        suspend(id)?;
        Ok(format!("{}+{}", self.label, other.label))
    }

    pub fn plain_touch(&self, id: u32) {
        suspend(id).expect("synthetic success");
    }
}

impl std::fmt::Display for Processor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        suspend(self.id).expect("synthetic display success");
        write!(f, "Processor({})", self.label)
    }
}

impl Drop for Processor {
    fn drop(&mut self) {
        DROPS.fetch_add(1, Ordering::SeqCst);
    }
}

#[uniffi::export]
pub fn drop_count() -> u32 {
    DROPS.load(Ordering::SeqCst)
}

#[derive(uniffi::Object)]
pub struct SyncObject;

#[uniffi::export]
impl SyncObject {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
    pub fn value(&self) -> u32 {
        7
    }
}
