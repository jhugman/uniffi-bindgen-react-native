/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

#[cfg(target_arch = "wasm32")]
extern crate uniffi_runtime_wasm as _;

use ext_types_consumer::ConsumerRecord;
use ext_types_custom::{ANestedGuid, Guid, Ouid, Url};
use ext_types_remote::{RemoteEnum, RemoteObject, RemoteRecord};
use ext_types_shared::{
    SharedEnum, SharedObject, SharedProcMacroRecord, SharedRecord, SharedTrait,
};
use std::sync::Arc;

// Remote types require a macro call in the Rust source
uniffi::use_remote_type!(ext_types_custom::Url);

pub struct CombinedType {
    pub shared_enum: SharedEnum,
    pub shared_record: SharedRecord,
    pub shared_records: Vec<SharedRecord>,
    pub maybe_shared_record: Option<SharedRecord>,

    pub guid: Guid,
    pub guids: Vec<Guid>,
    pub maybe_guid: Option<Guid>,

    pub url: Url,
    pub urls: Vec<Url>,
    pub maybe_url: Option<Url>,

    pub remote_record: RemoteRecord,
    pub remote_enum: RemoteEnum,
}

fn get_combined_type(existing: Option<CombinedType>) -> CombinedType {
    existing.unwrap_or_else(|| CombinedType {
        shared_enum: SharedEnum::One,
        shared_record: SharedRecord {
            sval: "hello".to_string(),
        },
        shared_records: vec![
            SharedRecord {
                sval: "first of many".to_string(),
            },
            SharedRecord {
                sval: "second of many".to_string(),
            },
        ],
        maybe_shared_record: None,

        guid: Guid("a-guid".into()),
        guids: vec![Guid("b-guid".into()), Guid("c-guid".into())],
        maybe_guid: None,

        url: Url::parse("http://example.com/").unwrap(),
        urls: vec![],
        maybe_url: None,

        remote_record: RemoteRecord {
            sval: "remote".into(),
        },
        remote_enum: RemoteEnum::One,
    })
}

// Not part of CombinedType as (a) object refs prevent equality testing and
// (b) it's not currently possible to refer to external traits in UDL.
#[derive(Default, uniffi::Record)]
pub struct ObjectsType {
    pub maybe_trait: Option<Arc<dyn SharedTrait>>,
    pub maybe_object: Option<Arc<SharedObject>>,
    pub consumer: ConsumerRecord,
}

#[uniffi::export]
fn get_objects_type(value: Option<ObjectsType>) -> ObjectsType {
    value.unwrap_or_default()
}

// A Custom type
fn get_url(url: Url) -> Url {
    url
}

fn get_urls(urls: Vec<Url>) -> Vec<Url> {
    urls
}

fn get_maybe_url(url: Option<Url>) -> Option<Url> {
    url
}

fn get_maybe_urls(urls: Vec<Option<Url>>) -> Vec<Option<Url>> {
    urls
}

#[uniffi::export]
fn get_imported_ouid(ouid: Ouid) -> Ouid {
    ouid
}

// external custom types wrapping external custom types.
#[uniffi::export]
fn get_imported_nested_guid(guid: Option<ANestedGuid>) -> ANestedGuid {
    guid.unwrap_or_else(|| ANestedGuid(Guid("nested".to_string())))
}

// A local custom type wrapping an external imported procmacro type
pub struct NestedExternalOuid(pub Ouid);
uniffi::custom_newtype!(NestedExternalOuid, Ouid);

#[uniffi::export]
fn get_nested_external_ouid(ouid: Option<NestedExternalOuid>) -> NestedExternalOuid {
    ouid.unwrap_or_else(|| NestedExternalOuid(Ouid("nested-external-ouid".to_string())))
}

// A struct
fn get_shared_record(t: SharedRecord) -> SharedRecord {
    t
}

fn get_shared_records(ts: Vec<SharedRecord>) -> Vec<SharedRecord> {
    ts
}

fn get_maybe_shared_record(t: Option<SharedRecord>) -> Option<SharedRecord> {
    t
}

// An enum
fn get_shared_enum(e: SharedEnum) -> SharedEnum {
    e
}

fn get_shared_enums(es: Vec<SharedEnum>) -> Vec<SharedEnum> {
    es
}

fn get_maybe_shared_enum(e: Option<SharedEnum>) -> Option<SharedEnum> {
    e
}

fn get_maybe_shared_enums(es: Vec<Option<SharedEnum>>) -> Vec<Option<SharedEnum>> {
    es
}

fn get_shared_object() -> Arc<SharedObject> {
    Arc::new(SharedObject::new())
}

#[uniffi::export]
fn get_shared_trait(t: Option<Arc<dyn SharedTrait>>) -> Option<Arc<dyn SharedTrait>> {
    t
}

fn get_shared_proc_macro_record(t: SharedProcMacroRecord) -> SharedProcMacroRecord {
    t
}

fn get_remote_object(val: String) -> Arc<RemoteObject> {
    Arc::new(RemoteObject::new(val))
}

uniffi::include_scaffolding!("ext_types");
