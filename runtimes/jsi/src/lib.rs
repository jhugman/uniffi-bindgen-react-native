/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! uniffi-runtime-jsi: a generic JSI player. Wraps `uniffi-runtime-core` behind a
//! small hand-written C ABI that the React Native C++ shim calls.

mod c_api;

pub use c_api::{
    ubrn_jsi_build_vtable, ubrn_jsi_call, ubrn_jsi_call_callback_ptr, ubrn_jsi_free,
    ubrn_jsi_make_trampoline, ubrn_jsi_register, ubrn_jsi_rustbuffer_alloc,
    ubrn_jsi_rustbuffer_free, ubrn_jsi_rustbuffer_from_bytes, ubrn_jsi_struct_field_offsets,
    UbrnCallbackSpec, UbrnFunctionSpec, UbrnJsiModule, UbrnModuleSpec, UbrnStructField,
    UbrnStructSpec,
};
