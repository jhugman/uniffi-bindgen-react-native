/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

mod platform_specific;
use platform_specific::acquire_with_timeout;
use ubrn_testing::timer::{TimerFuture, TimerService};

use std::{sync::Arc, time::Duration};

/// Sync function.
#[uniffi::export]
pub fn greet(who: String) -> String {
    format!("Hello, {who}")
}

/// Async function that is immediately ready.
///
/// (This one is defined in the UDL to test UDL support)
pub async fn always_ready() -> bool {
    true
}

#[uniffi::export]
pub async fn void() {}

/// Async function that says something after 2s.
#[uniffi::export]
pub async fn say() -> String {
    TimerFuture::sleep(Duration::from_secs(2)).await;

    "Hello, Future!".to_string()
}

/// Async function that says something after a certain time.
#[uniffi::export]
pub async fn say_after(ms: u16, who: String) -> String {
    TimerFuture::sleep(Duration::from_millis(ms.into())).await;

    format!("Hello, {who}!")
}

/// Async function that sleeps!
#[uniffi::export]
pub async fn sleep(ms: u16) -> bool {
    TimerFuture::sleep(Duration::from_millis(ms.into())).await;

    true
}

/// Async function that sleeps with no return type
#[uniffi::export]
pub async fn sleep_no_return(ms: u16) {
    TimerFuture::sleep(Duration::from_millis(ms.into())).await;
}

// Our error.
#[derive(thiserror::Error, uniffi::Error, Debug)]
pub enum MyError {
    #[error("Foo")]
    Foo,
}

// An async function that can throw.
#[uniffi::export]
pub async fn fallible_me(do_fail: bool) -> Result<u8, MyError> {
    if do_fail {
        Err(MyError::Foo)
    } else {
        Ok(42)
    }
}

// An async function returning a struct that can throw.
#[uniffi::export]
pub async fn fallible_struct(do_fail: bool) -> Result<Arc<Megaphone>, MyError> {
    if do_fail {
        Err(MyError::Foo)
    } else {
        Ok(new_megaphone())
    }
}

/// Sync function that generates a new `Megaphone`.
///
/// It builds a `Megaphone` which has async methods on it.
#[uniffi::export]
pub fn new_megaphone() -> Arc<Megaphone> {
    Arc::new(Megaphone)
}

/// Async function that generates a new `Megaphone`.
#[uniffi::export]
pub async fn async_new_megaphone() -> Arc<Megaphone> {
    new_megaphone()
}

/// Async function that possibly generates a new `Megaphone`.
#[uniffi::export]
pub async fn async_maybe_new_megaphone(y: bool) -> Option<Arc<Megaphone>> {
    if y {
        Some(new_megaphone())
    } else {
        None
    }
}

/// Async function that inputs `Megaphone`.
#[uniffi::export]
pub async fn say_after_with_megaphone(megaphone: Arc<Megaphone>, ms: u16, who: String) -> String {
    megaphone.say_after(ms, who).await
}

/// A megaphone. Be careful with the neighbours.
#[derive(uniffi::Object)]
pub struct Megaphone;

#[uniffi::export]
impl Megaphone {
    // the default constructor - many bindings will not support this.
    #[uniffi::constructor]
    pub async fn new() -> Arc<Self> {
        TimerFuture::sleep(Duration::from_millis(0)).await;
        Arc::new(Self {})
    }

    // most should support this.
    #[uniffi::constructor]
    pub async fn secondary() -> Arc<Self> {
        TimerFuture::sleep(Duration::from_millis(0)).await;
        Arc::new(Self {})
    }

    /// An async method that yells something after a certain time.
    pub async fn say_after(self: Arc<Self>, ms: u16, who: String) -> String {
        say_after(ms, who).await.to_uppercase()
    }

    /// An async method without any extra arguments.
    pub async fn silence(&self) -> String {
        String::new()
    }

    /// An async method that can throw.
    pub async fn fallible_me(self: Arc<Self>, do_fail: bool) -> Result<u8, MyError> {
        if do_fail {
            Err(MyError::Foo)
        } else {
            Ok(42)
        }
    }
}

#[derive(uniffi::Object)]
pub struct FallibleMegaphone;

#[uniffi::export]
impl FallibleMegaphone {
    // the default constructor - many bindings will not support this.
    #[uniffi::constructor]
    pub async fn new() -> Result<Arc<Self>, MyError> {
        Err(MyError::Foo)
    }
}

pub struct UdlMegaphone;

impl UdlMegaphone {
    pub async fn new() -> Self {
        Self {}
    }

    pub async fn secondary() -> Self {
        Self {}
    }

    pub async fn say_after(self: Arc<Self>, ms: u16, who: String) -> String {
        say_after(ms, who).await.to_uppercase()
    }
}

#[derive(uniffi::Record)]
pub struct MyRecord {
    pub a: String,
    pub b: u32,
}

#[uniffi::export]
pub async fn new_my_record(a: String, b: u32) -> MyRecord {
    MyRecord { a, b }
}

#[derive(uniffi::Record)]
pub struct SharedResourceOptions {
    pub release_after_ms: u16,
    pub timeout_ms: u16,
}

// Our error.
#[derive(thiserror::Error, uniffi::Error, Debug)]
pub enum AsyncError {
    #[error("Timeout")]
    Timeout,
}

#[cfg_attr(target_arch = "wasm32", uniffi::export)]
#[cfg_attr(not(target_arch = "wasm32"), uniffi::export(async_runtime = "tokio"))]
pub async fn use_shared_resource(options: SharedResourceOptions) -> Result<(), AsyncError> {
    acquire_with_timeout(options).await
}

uniffi::include_scaffolding!("async-calls");
