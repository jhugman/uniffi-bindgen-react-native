/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
type EventHandlerFut = Pin<Box<dyn Future<Output = ()> + Send>>;
#[cfg(target_arch = "wasm32")]
type EventHandlerFut = Pin<Box<dyn Future<Output = ()>>>;

#[cfg(not(target_arch = "wasm32"))]
type EventHandlerFn = dyn Fn(u64) -> EventHandlerFut + Send + Sync;
#[cfg(target_arch = "wasm32")]
type EventHandlerFn = dyn Fn(u64) -> EventHandlerFut;


// Async callback interface implemented in foreign code
#[derive(uniffi::Object)]
pub struct SimpleObject {
  inner: Arc<InnerObject>
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
enum ClientError {
  #[error("Password is too weak")]
  Error(String),
}

#[uniffi::export]
impl SimpleObject {
  async fn builder_pattern(&self, key: String) -> Result<Arc<SimpleObject>, ClientError> {
    Ok(Arc::new(SimpleObject {
        inner: Arc::new(InnerObject {
            foo: Foo {
                key,
                value: 42,
            },
            callbacks: vec![],
        }),
    }))
  }
}

pub struct InnerObject {
  pub foo: Foo,
  pub callbacks: Vec<Box<EventHandlerFn>>,
}

pub struct Foo {
  pub key: String,
  pub value: u64,
}

#[uniffi::export]
async fn make_object() -> Arc<SimpleObject> {
    Arc::new(SimpleObject {
        inner: Arc::new(InnerObject {
            foo: Foo {
                key: "key".to_string(),
                value: 42,
            },
            callbacks: vec![],
        }),
    })
}

uniffi::setup_scaffolding!();
