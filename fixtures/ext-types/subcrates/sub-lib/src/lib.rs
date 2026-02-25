/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::sync::Arc;
use uniffi_one::{UniffiOneEnum, UniffiOneError, UniffiOneInterface, UniffiOneTrait};

#[derive(Default, uniffi::Record)]
pub struct SubLibType {
    pub maybe_enum: Option<UniffiOneEnum>,
    pub maybe_trait: Option<Arc<dyn UniffiOneTrait>>,
    pub maybe_interface: Option<Arc<UniffiOneInterface>>,
}

#[uniffi::export]
fn get_sub_type(existing: Option<SubLibType>) -> SubLibType {
    existing.unwrap_or_default()
}

struct OneImpl;

impl UniffiOneTrait for OneImpl {
    fn hello(&self) -> String {
        "sub-lib trait impl says hello".to_string()
    }
}

#[uniffi::export]
fn get_trait_impl() -> Arc<dyn UniffiOneTrait> {
    Arc::new(OneImpl {})
}

#[uniffi::export]
fn call_trait_impl(t: Arc<dyn UniffiOneTrait>) -> String {
    t.hello()
}

#[uniffi::export]
fn throw_uniffi_one_error() -> Result<(), UniffiOneError> {
    Err(UniffiOneError::TheError)
}

uniffi::setup_scaffolding!("imported_types_sublib");
