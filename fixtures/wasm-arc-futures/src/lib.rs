/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ubrn_testing::timer::{TimerFuture, TimerService};

#[cfg(not(target_arch = "wasm32"))]
type EventHandlerFut = Pin<Box<dyn Future<Output = ()> + Send>>;
#[cfg(target_arch = "wasm32")]
type EventHandlerFut = Pin<Box<dyn Future<Output = ()>>>;

#[cfg(not(target_arch = "wasm32"))]
type EventHandlerFn = dyn Fn(String, String) -> EventHandlerFut + Send + Sync;
#[cfg(target_arch = "wasm32")]
type EventHandlerFn = dyn Fn(String, String) -> EventHandlerFut;

#[derive(uniffi::Object)]
pub struct SimpleObject {
    inner: Mutex<String>,
    callbacks: Vec<Box<EventHandlerFn>>,
}

impl fmt::Debug for SimpleObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SimpleObject")
    }
}

impl fmt::Display for SimpleObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl SimpleObject {
    #[cfg_attr(target_arch = "wasm32", allow(clippy::arc_with_non_send_sync))]
    fn new_with_callback(cb: Box<EventHandlerFn>) -> Arc<Self> {
        Arc::new(SimpleObject {
            inner: Mutex::new("key".to_string()),
            callbacks: vec![cb],
        })
    }
}

#[uniffi::export]
impl SimpleObject {
    pub async fn update(self: Arc<Self>, updated: String) {
        let old = {
            let mut data = self.inner.lock().unwrap();
            let old = data.clone();
            *data = updated.clone();
            old
        };
        for callback in self.callbacks.iter() {
            callback(old.clone(), updated.clone()).await;
        }
    }
}

pub async fn wait(_old: String, _new: String) {
    TimerFuture::sleep(Duration::from_millis(200)).await;
}

fn from_static() -> Box<EventHandlerFn> {
    Box::new(|old, new| Box::pin(wait(old, new)))
}

// Make an object, with no callbacks.
// This relies on a timer, which is implemented for wasm using gloo.
// This is not Send, so EventHandlerFn and EventHandlerFut are different
// for wasm.
#[uniffi::export]
async fn make_object() -> Arc<SimpleObject> {
    SimpleObject::new_with_callback(from_static())
}

#[uniffi::export]
async fn throw_object() -> Result<(), Arc<SimpleObject>> {
    let obj = make_object().await;
    Err(obj)
}

// Simple callback interface object, with a synchronous method.
// The foreign trait isn't asynchronous, so we shouldn't be seeing
// any problem here.
#[uniffi::export(with_foreign)]
pub trait SimpleCallback: Sync + Send {
    fn on_update(&self, previous: String, current: String);
}

#[uniffi::export]
async fn simple_callback(callback: Arc<dyn SimpleCallback>) -> Arc<dyn SimpleCallback> {
    callback
}

fn from_simple_callback(callback: Arc<dyn SimpleCallback>) -> Box<EventHandlerFn> {
    Box::new(move |old: String, new: String| {
        let callback = callback.clone();
        Box::pin(async move {
            callback.on_update(old, new);
        })
    })
}

#[uniffi::export]
async fn make_object_with_callback(callback: Arc<dyn SimpleCallback>) -> Arc<SimpleObject> {
    SimpleObject::new_with_callback(from_simple_callback(callback))
}

// An async callback interface; the async foreign trait will be
// a Send and Sync, so this shouldn't be testing anything new.
#[cfg(target_arch = "wasm32")]
#[uniffi::export(with_foreign)]
#[async_trait::async_trait(?Send)]
pub trait AsyncCallback {
    async fn on_update(&self, previous: String, current: String);
}

#[cfg(not(target_arch = "wasm32"))]
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait AsyncCallback: Send + Sync {
    async fn on_update(&self, previous: String, current: String);
}

#[uniffi::export]
async fn async_callback(callback: Arc<dyn AsyncCallback>) -> Arc<dyn AsyncCallback> {
    callback
}

fn from_async_callback(callback: Arc<dyn AsyncCallback>) -> Box<EventHandlerFn> {
    Box::new(move |old: String, new: String| {
        let callback = callback.clone();
        Box::pin(async move {
            // Look, there's an .await here.
            callback.on_update(old, new).await;
        })
    })
}

#[uniffi::export]
async fn make_object_with_async_callback(callback: Arc<dyn AsyncCallback>) -> Arc<SimpleObject> {
    SimpleObject::new_with_callback(from_async_callback(callback))
}

// Rust only trait
#[cfg(not(target_arch = "wasm32"))]
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait RustCallback: Sync + Send {
    async fn on_update(&self, previous: String, current: String) -> String;
}

#[cfg(target_arch = "wasm32")]
#[uniffi::export(with_foreign)]
#[async_trait::async_trait(?Send)]
pub trait RustCallback {
    async fn on_update(&self, previous: String, current: String) -> String;
}

struct NoopRustCallback;

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl RustCallback for NoopRustCallback {
    async fn on_update(&self, previous: String, current: String) -> String {
        use std::time::Duration;
        use ubrn_testing::timer::{TimerFuture, TimerService};
        TimerFuture::sleep(Duration::from_millis(200)).await;
        format!("{previous} -> {current}")
    }
}

#[uniffi::export]
async fn rust_callback() -> Arc<dyn RustCallback> {
    Arc::new(NoopRustCallback)
}

uniffi::setup_scaffolding!();
