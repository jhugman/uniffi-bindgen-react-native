/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::{
    sync::Arc,
    time::Duration,
};

use ubrn_testing::timer::{TimerFuture, TimerService};
use futures::future::{AbortHandle, Abortable, Aborted};

// Example of an trait with async methods
#[uniffi::export]
#[async_trait::async_trait]
pub trait SayAfterTrait: Send + Sync {
    async fn say_after(&self, ms: u16, who: String) -> String;
}

struct SayAfterImpl1;

struct SayAfterImpl2;

#[async_trait::async_trait]
impl SayAfterTrait for SayAfterImpl1 {
    async fn say_after(&self, ms: u16, who: String) -> String {
        say_after(ms, who).await
    }
}

#[async_trait::async_trait]
impl SayAfterTrait for SayAfterImpl2 {
    async fn say_after(&self, ms: u16, who: String) -> String {
        say_after(ms, who).await
    }
}

#[uniffi::export]
fn get_say_after_traits() -> Vec<Arc<dyn SayAfterTrait>> {
    vec![Arc::new(SayAfterImpl1), Arc::new(SayAfterImpl2)]
}

// Async callback interface implemented in foreign code
#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait AsyncParser: Send + Sync {
    // Simple async method
    async fn as_string(&self, delay_ms: i32, value: i32) -> String;
    // Async method that can throw
    async fn try_from_string(&self, delay_ms: i32, value: String) -> Result<i32, ParserError>;
    // Void return, which requires special handling
    async fn delay(&self, delay_ms: i32);
    // Void return that can also throw
    async fn try_delay(&self, delay_ms: String) -> Result<(), ParserError>;
}

#[derive(thiserror::Error, uniffi::Error, Debug)]
pub enum ParserError {
    #[error("NotAnInt")]
    NotAnInt,
    #[error("UnexpectedError")]
    UnexpectedError,
}

impl From<uniffi::UnexpectedUniFFICallbackError> for ParserError {
    fn from(_: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::UnexpectedError
    }
}

#[uniffi::export]
async fn as_string_using_trait(obj: Arc<dyn AsyncParser>, delay_ms: i32, value: i32) -> String {
    obj.as_string(delay_ms, value).await
}

#[uniffi::export]
async fn try_from_string_using_trait(
    obj: Arc<dyn AsyncParser>,
    delay_ms: i32,
    value: String,
) -> Result<i32, ParserError> {
    obj.try_from_string(delay_ms, value).await
}

#[uniffi::export]
async fn delay_using_trait(obj: Arc<dyn AsyncParser>, delay_ms: i32) {
    obj.delay(delay_ms).await
}

#[uniffi::export]
async fn try_delay_using_trait(
    obj: Arc<dyn AsyncParser>,
    delay_ms: String,
) -> Result<(), ParserError> {
    obj.try_delay(delay_ms).await
}

#[uniffi::export]
async fn cancel_delay_using_trait(obj: Arc<dyn AsyncParser>, delay_ms: i32) {
    let (abort_handle, abort_registration) = AbortHandle::new_pair();
    let aborter = sleep_then_abort(abort_handle);

    let future = Abortable::new(obj.delay(delay_ms), abort_registration);

    let (_, delay_result) = futures::join!(aborter, future);

    // Check the result of the aborted future
    assert_eq!(delay_result, Err(Aborted));
}

async fn sleep_then_abort(abort_handle: AbortHandle) {
    TimerFuture::sleep(Duration::from_millis(1)).await;
    abort_handle.abort();
}

/// Async function that says something after a certain time.
#[uniffi::export]
pub async fn say_after(ms: u16, who: String) -> String {
    TimerFuture::sleep(Duration::from_millis(ms.into())).await;

    format!("Hello, {who}!")
}

uniffi::setup_scaffolding!();
