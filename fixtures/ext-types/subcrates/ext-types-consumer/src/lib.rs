/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use ext_types_shared::{SharedEnum, SharedError, SharedObject, SharedTrait};
use std::sync::Arc;

#[derive(Default, uniffi::Record)]
pub struct ConsumerRecord {
    pub maybe_enum: Option<SharedEnum>,
    pub maybe_trait: Option<Arc<dyn SharedTrait>>,
    pub maybe_object: Option<Arc<SharedObject>>,
}

#[uniffi::export]
fn get_consumer_record(existing: Option<ConsumerRecord>) -> ConsumerRecord {
    existing.unwrap_or_default()
}

struct ConsumerTraitImpl;

#[async_trait::async_trait]
impl SharedTrait for ConsumerTraitImpl {
    async fn hello(&self) -> String {
        "consumer trait impl says hello".to_string()
    }
}

#[uniffi::export]
fn get_trait_impl() -> Arc<dyn SharedTrait> {
    Arc::new(ConsumerTraitImpl {})
}

#[uniffi::export]
async fn call_trait_impl(t: Arc<dyn SharedTrait>) -> String {
    t.hello().await
}

#[uniffi::export]
fn throw_shared_error() -> Result<(), SharedError> {
    Err(SharedError::TheError)
}

uniffi::setup_scaffolding!("ext_types_consumer");
