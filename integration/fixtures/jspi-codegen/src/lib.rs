// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
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

fn suspend(id: u32) -> Result<u32, FixtureError> {
    #[cfg(target_arch = "wasm32")]
    {
        request(id).map_err(|_| FixtureError::Rejected)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = id;
        unreachable!("only called in the wasm fixture")
    }
}

#[uniffi::export]
pub fn compute(id: u32, rounds: u32) -> Result<u32, FixtureError> {
    let mut stack_data = [id; 256];
    let mut total = 0;
    for _ in 0..rounds {
        total += suspend(id)?;
        assert!(std::hint::black_box(&mut stack_data)
            .iter()
            .all(|v| *v == id));
    }
    Ok(total)
}

#[uniffi::export]
pub fn bytes(id: u32, value: Vec<u8>) -> Result<Vec<u8>, FixtureError> {
    suspend(id)?;
    Ok(value)
}

#[uniffi::export]
pub fn text(id: u32, value: String) -> Result<String, FixtureError> {
    suspend(id)?;
    Ok(value)
}

#[derive(Clone, uniffi::Record)]
pub struct Payload {
    pub label: String,
    pub value: u32,
}

#[uniffi::export]
impl Payload {
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

// Rust async bodies suspend during UniFFI polling, not future creation.
#[uniffi::export]
pub async fn async_bytes(id: u32, pending_after: bool) -> Result<Vec<u8>, FixtureError> {
    let value = suspend(id)?;
    if pending_after {
        let mut first = true;
        std::future::poll_fn(|cx| {
            if first {
                first = false;
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            } else {
                std::task::Poll::Ready(())
            }
        })
        .await;
    }
    Ok(format!("async:{value}").into_bytes())
}

#[uniffi::export]
pub async fn async_void(id: u32) -> Result<(), FixtureError> {
    suspend(id)?;
    Ok(())
}

#[uniffi::export]
pub async fn unselected_async(value: u32) -> u32 {
    value + 1
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

#[uniffi::export]
pub fn record(id: u32, value: Payload) -> Result<Payload, FixtureError> {
    suspend(id)?;
    Ok(value)
}

#[uniffi::export]
pub fn complete_void(id: u32) -> Result<(), FixtureError> {
    suspend(id)?;
    Ok(())
}

#[uniffi::export]
pub fn synchronous(value: u32) -> u32 {
    value + 1
}

#[uniffi::export]
pub fn plain_scalar(id: u32) -> u32 {
    suspend(id).expect("synthetic success")
}

#[uniffi::export]
pub fn plain_void(id: u32) {
    suspend(id).expect("synthetic success");
}

uniffi::setup_scaffolding!();

#[cfg(all(target_arch = "wasm32", feature = "bindings"))]
#[path = "../generated/rs/jspi_codegen_module.rs"]
mod bindings;
