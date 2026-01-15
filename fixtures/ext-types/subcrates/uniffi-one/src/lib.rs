/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{
    fmt::Display,
    sync::atomic::{AtomicI32, Ordering},
};

pub struct UniffiOneType {
    pub sval: String,
}

pub enum UniffiOneEnum {
    One,
    Two,
}

#[derive(Debug, uniffi::Error)]
pub enum UniffiOneError {
    TheError,
}

impl Display for UniffiOneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{self:?}"))
    }
}

#[derive(uniffi::Record)]
pub struct UniffiOneProcMacroType {
    pub sval: String,
}

#[derive(Default)]
pub struct UniffiOneInterface {
    current: AtomicI32,
}

impl UniffiOneInterface {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn increment(&self) -> i32 {
        self.current.fetch_add(1, Ordering::Relaxed) + 1
    }
}

#[uniffi::export]
fn get_my_proc_macro_type(t: UniffiOneProcMacroType) -> UniffiOneProcMacroType {
    t
}

#[uniffi::export]
async fn get_uniffi_one_async() -> UniffiOneEnum {
    UniffiOneEnum::One
}

#[uniffi::export(with_foreign)]
pub trait UniffiOneTrait: Send + Sync {
    fn hello(&self) -> String;
}

// Note `UDL` vs `Udl` is important here to test foreign binding name fixups.
pub trait UniffiOneUDLTrait: Send + Sync {
    fn hello(&self) -> String;

    #[doc(hidden)]
    fn uniffi_foreign_handle(&self) -> Option<uniffi::Handle> {
        None
    }
}

uniffi::include_scaffolding!("uniffi-one");
