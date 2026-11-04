/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{
    fmt::Display,
    sync::atomic::{AtomicI32, Ordering},
};

pub struct SharedRecord {
    pub sval: String,
}

pub enum SharedEnum {
    One,
    Two,
}

#[derive(Debug, uniffi::Error)]
pub enum SharedError {
    TheError,
}

impl Display for SharedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{self:?}"))
    }
}

#[derive(uniffi::Record)]
pub struct SharedProcMacroRecord {
    pub sval: String,
}

#[derive(Default)]
pub struct SharedObject {
    current: AtomicI32,
}

impl SharedObject {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn increment(&self) -> i32 {
        self.current.fetch_add(1, Ordering::Relaxed) + 1
    }
}

#[uniffi::export]
fn get_proc_macro_record(t: SharedProcMacroRecord) -> SharedProcMacroRecord {
    t
}

#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait SharedTrait: Send + Sync {
    async fn hello(&self) -> String;
}

uniffi::include_scaffolding!("ext_types_shared");
