/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Compile-time ABI drift guard.
//
// `runtimes/jsi/include/ubrn_jsi.h`, `runtimes/jsi/src/c_api.rs`, and
// `runtimes/core`'s `#[repr(C)]` structs are maintained BY HAND and must mirror
// each other exactly (field order/sizes, enum tag values). A silent divergence
// — a field reordered, a tag renumbered — is memory corruption. This
// translation unit `static_assert`s those invariants so any drift fails the
// build.
//
// The Rust side pins the same facts in `c_api.rs`'s `#[cfg(test)] mod tests`.
#include "ubrn_jsi.h"

#include <cstddef>
#include <cstdint>

// --- UbrnRustBuffer must match core::RustBufferC (ffi_c_types.rs) ------------
// `{ u64 capacity; u64 len; *mut u8 data }` — 24 bytes; capacity@0, len@8,
// data@16.
static_assert(sizeof(UbrnRustBuffer) == 24, "RustBuffer size must be 24 bytes");
static_assert(offsetof(UbrnRustBuffer, capacity) == 0,
              "RustBuffer.capacity @0");
static_assert(offsetof(UbrnRustBuffer, len) == 8, "RustBuffer.len @8");
static_assert(offsetof(UbrnRustBuffer, data) == 16, "RustBuffer.data @16");

// --- Type tag values must match c_api.rs::ffi_type_from_tag /
// core::FfiTypeDesc. Scalars (Void=0 .. Handle=11).
static_assert(UBRN_TY_VOID == 0, "tag Void");
static_assert(UBRN_TY_U8 == 1, "tag U8");
static_assert(UBRN_TY_I8 == 2, "tag I8");
static_assert(UBRN_TY_U16 == 3, "tag U16");
static_assert(UBRN_TY_I16 == 4, "tag I16");
static_assert(UBRN_TY_U32 == 5, "tag U32");
static_assert(UBRN_TY_I32 == 6, "tag I32");
static_assert(UBRN_TY_U64 == 7, "tag U64");
static_assert(UBRN_TY_I64 == 8, "tag I64");
static_assert(UBRN_TY_F32 == 9, "tag F32");
static_assert(UBRN_TY_F64 == 10, "tag F64");
static_assert(UBRN_TY_HANDLE == 11, "tag Handle");
// Named / compound tags (the drift-prone ones).
static_assert(UBRN_TY_RUSTBUFFER == 12, "tag RustBuffer");
static_assert(UBRN_TY_CALLBACK == 13, "tag Callback");
static_assert(UBRN_TY_STRUCT == 14, "tag Struct");
static_assert(UBRN_TY_REFERENCE == 15, "tag Reference");
static_assert(UBRN_TY_RUSTCALLSTATUS == 16, "tag RustCallStatus");

// --- Spec struct layouts must match c_api.rs's #[repr(C)] mirrors ------------
// These structs are passed by pointer from JS-built native memory into Rust; a
// field reorder on either side corrupts every registration. Offsets are the
// natural #[repr(C)] layout on a 64-bit target (8-byte pointers/size_t).

// UbrnFunctionSpec { *name, *arg_tags, size_t n_args, *arg_type_names,
//                    u8 ret_tag, u8 has_rust_call_status }.
static_assert(offsetof(UbrnFunctionSpec, name) == 0, "FunctionSpec.name @0");
static_assert(offsetof(UbrnFunctionSpec, arg_tags) == 8,
              "FunctionSpec.arg_tags @8");
static_assert(offsetof(UbrnFunctionSpec, n_args) == 16,
              "FunctionSpec.n_args @16");
static_assert(offsetof(UbrnFunctionSpec, arg_type_names) == 24,
              "FunctionSpec.arg_type_names @24");
static_assert(offsetof(UbrnFunctionSpec, ret_tag) == 32,
              "FunctionSpec.ret_tag @32");
static_assert(offsetof(UbrnFunctionSpec, has_rust_call_status) == 33,
              "FunctionSpec.has_rust_call_status @33");
static_assert(sizeof(UbrnFunctionSpec) == 40, "FunctionSpec size");

// UbrnCallbackSpec { *name, *arg_tags, *arg_type_names, size_t n_args,
//                    u8 ret_tag, u8 has_rust_call_status, u8 out_return,
//                    *ret_type_name }.
static_assert(offsetof(UbrnCallbackSpec, name) == 0, "CallbackSpec.name @0");
static_assert(offsetof(UbrnCallbackSpec, arg_tags) == 8,
              "CallbackSpec.arg_tags @8");
static_assert(offsetof(UbrnCallbackSpec, arg_type_names) == 16,
              "CallbackSpec.arg_type_names @16");
static_assert(offsetof(UbrnCallbackSpec, n_args) == 24,
              "CallbackSpec.n_args @24");
static_assert(offsetof(UbrnCallbackSpec, ret_tag) == 32,
              "CallbackSpec.ret_tag @32");
static_assert(offsetof(UbrnCallbackSpec, has_rust_call_status) == 33,
              "CallbackSpec.has_rust_call_status @33");
static_assert(offsetof(UbrnCallbackSpec, out_return) == 34,
              "CallbackSpec.out_return @34");
static_assert(offsetof(UbrnCallbackSpec, ret_type_name) == 40,
              "CallbackSpec.ret_type_name @40");
static_assert(sizeof(UbrnCallbackSpec) == 48, "CallbackSpec size");

// UbrnStructField { *field_name, u8 type_tag, *type_name }.
static_assert(offsetof(UbrnStructField, field_name) == 0,
              "StructField.field_name @0");
static_assert(offsetof(UbrnStructField, type_tag) == 8,
              "StructField.type_tag @8");
static_assert(offsetof(UbrnStructField, type_name) == 16,
              "StructField.type_name @16");
static_assert(sizeof(UbrnStructField) == 24, "StructField size");

// UbrnStructSpec { *name, *fields, size_t n_fields }.
static_assert(offsetof(UbrnStructSpec, name) == 0, "StructSpec.name @0");
static_assert(offsetof(UbrnStructSpec, fields) == 8, "StructSpec.fields @8");
static_assert(offsetof(UbrnStructSpec, n_fields) == 16,
              "StructSpec.n_fields @16");
static_assert(sizeof(UbrnStructSpec) == 24, "StructSpec size");

// UbrnModuleSpec { *rustbuffer_alloc, *rustbuffer_free, *rustbuffer_from_bytes,
//                  *functions, size_t n_functions, *callbacks, size_t
//                  n_callbacks, *structs, size_t n_structs }.
static_assert(offsetof(UbrnModuleSpec, rustbuffer_alloc) == 0,
              "ModuleSpec.rustbuffer_alloc @0");
static_assert(offsetof(UbrnModuleSpec, rustbuffer_free) == 8,
              "ModuleSpec.rustbuffer_free @8");
static_assert(offsetof(UbrnModuleSpec, rustbuffer_from_bytes) == 16,
              "ModuleSpec.rustbuffer_from_bytes @16");
static_assert(offsetof(UbrnModuleSpec, functions) == 24,
              "ModuleSpec.functions @24");
static_assert(offsetof(UbrnModuleSpec, n_functions) == 32,
              "ModuleSpec.n_functions @32");
static_assert(offsetof(UbrnModuleSpec, callbacks) == 40,
              "ModuleSpec.callbacks @40");
static_assert(offsetof(UbrnModuleSpec, n_callbacks) == 48,
              "ModuleSpec.n_callbacks @48");
static_assert(offsetof(UbrnModuleSpec, structs) == 56,
              "ModuleSpec.structs @56");
static_assert(offsetof(UbrnModuleSpec, n_structs) == 64,
              "ModuleSpec.n_structs @64");
static_assert(sizeof(UbrnModuleSpec) == 72, "ModuleSpec size");
