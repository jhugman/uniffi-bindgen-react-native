/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::sync::Arc;

// Named in forceAsync -> async surface.
#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct AsyncObj {
    val: String,
}

#[uniffi::export]
impl AsyncObj {
    #[uniffi::constructor]
    pub fn new(val: String) -> Arc<Self> {
        Arc::new(Self { val })
    }

    pub fn label(&self) -> String {
        self.val.clone()
    }
}

impl std::fmt::Display for AsyncObj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AsyncObj({})", self.val)
    }
}

// NOT named in forceAsync -> synchronous surface.
#[derive(uniffi::Object)]
#[uniffi::export(Display)]
pub struct SyncObj {
    val: String,
}

#[uniffi::export]
impl SyncObj {
    #[uniffi::constructor]
    pub fn new(val: String) -> Arc<Self> {
        Arc::new(Self { val })
    }

    pub fn label(&self) -> String {
        self.val.clone()
    }
}

impl std::fmt::Display for SyncObj {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SyncObj({})", self.val)
    }
}

// `asyncFn` (TS name) is named in forceAsync -> async; `syncFn` is not.
#[uniffi::export]
pub fn async_fn(x: u32) -> u32 {
    x + 1
}

#[uniffi::export]
pub fn sync_fn(x: u32) -> u32 {
    x + 1
}

uniffi::setup_scaffolding!();
