/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use crate::{FlatWidget, Widget, WidgetEnum, WidgetRecord};

pub struct Handle(pub i64);
uniffi::custom_newtype!(Handle, i64);

// One value of every type family, so each shape below exercises all of them.
#[derive(uniffi::Record)]
pub struct Everything {
    pub flag: bool,
    pub small: i8,
    pub byte: u8,
    pub short: i16,
    pub ushort: u16,
    pub int: i32,
    pub uint: u32,
    pub long: i64,
    pub ulong: u64,
    pub single: f32,
    pub double: f64,
    pub text: String,
    pub bytes: Vec<u8>,
    pub when: SystemTime,
    pub span: Duration,
    pub maybe: Option<String>,
    pub list: Vec<i32>,
    pub map: HashMap<String, i32>,
    pub record: WidgetRecord,
    pub tagged: WidgetEnum,
    pub flat: FlatWidget,
    pub widget: Arc<Widget>,
    pub handle: Handle,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum FlatError {
    #[error("oops")]
    Oops,
}

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum EchoError {
    #[error("code {code}")]
    Code { code: i32 },
    #[error("{reason}")]
    Unexpected { reason: String },
}

impl From<uniffi::UnexpectedUniFFICallbackError> for EchoError {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        Self::Unexpected { reason: e.reason }
    }
}

#[uniffi::export]
pub fn echo(v: Everything) -> Everything {
    v
}

#[uniffi::export]
pub async fn echo_async(v: Everything) -> Everything {
    v
}

#[derive(uniffi::Object)]
pub struct Echoer;

#[uniffi::export]
impl Echoer {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    pub fn echo(&self, v: Everything) -> Everything {
        v
    }

    pub async fn echo_async(&self, v: Everything) -> Everything {
        v
    }
}

#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait ForeignEcho: Send + Sync {
    async fn echo(&self, v: Everything) -> Everything;
    async fn fail(&self, code: i32) -> Result<(), EchoError>;
}

#[uniffi::export]
pub async fn echo_via(cb: Arc<dyn ForeignEcho>, v: Everything) -> Everything {
    cb.echo(v).await
}

#[uniffi::export]
pub async fn fail_via(cb: Arc<dyn ForeignEcho>, code: i32) -> Result<(), EchoError> {
    cb.fail(code).await
}

#[uniffi::export]
pub async fn return_foreign(cb: Arc<dyn ForeignEcho>) -> Arc<dyn ForeignEcho> {
    cb
}

#[uniffi::export]
pub fn fail_flat() -> Result<(), FlatError> {
    Err(FlatError::Oops)
}

#[uniffi::export]
pub async fn fail_rich_async(code: i32) -> Result<(), EchoError> {
    Err(EchoError::Code { code })
}
