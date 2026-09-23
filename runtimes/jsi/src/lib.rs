/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! uniffi-runtime-jsi: a generic JSI player. Wraps `uniffi-runtime-core` behind a
//! small hand-written C ABI that the React Native C++ shim calls.

#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]

mod c_api;

pub use c_api::{
    ubrn_jsi_build_vtable, ubrn_jsi_call, ubrn_jsi_call_callback_ptr, ubrn_jsi_callback_arg_layout,
    ubrn_jsi_disarm, ubrn_jsi_free, ubrn_jsi_make_trampoline, ubrn_jsi_register,
    ubrn_jsi_remember_trampoline, ubrn_jsi_return_size, ubrn_jsi_rustbuffer_alloc,
    ubrn_jsi_rustbuffer_free, ubrn_jsi_rustbuffer_from_bytes, ubrn_jsi_scalar_slot_size_align,
    ubrn_jsi_struct_field_offsets, ubrn_jsi_trampoline_for, UbrnCallbackSpec, UbrnFunctionSpec,
    UbrnJsiModule, UbrnModuleSpec, UbrnStructField, UbrnStructSpec,
};
