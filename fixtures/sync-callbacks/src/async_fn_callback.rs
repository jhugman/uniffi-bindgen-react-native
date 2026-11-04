/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::sync::{Arc, Mutex};

// A sync foreign trait driven from async Rust functions.
#[uniffi::export(with_foreign)]
pub trait SimpleCallback: Sync + Send {
    fn on_update(&self, previous: String, current: String);
}

#[uniffi::export]
async fn simple_callback(callback: Arc<dyn SimpleCallback>) -> Arc<dyn SimpleCallback> {
    callback
}

/// An object that holds a foreign callback and reports every update through it.
#[derive(uniffi::Object)]
pub struct CallbackObject {
    inner: Mutex<String>,
    callback: Arc<dyn SimpleCallback>,
}

#[uniffi::export]
impl CallbackObject {
    pub async fn update(self: Arc<Self>, updated: String) {
        let old = std::mem::replace(&mut *self.inner.lock().unwrap(), updated.clone());
        self.callback.on_update(old, updated);
    }
}

#[uniffi::export]
async fn make_object_with_callback(callback: Arc<dyn SimpleCallback>) -> Arc<CallbackObject> {
    Arc::new(CallbackObject {
        inner: Mutex::new("key".to_string()),
        callback,
    })
}
