/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Compile-time ABI drift guard.
//
// `runtimes/jsi/include/ubrn_jsi.h`, `runtimes/jsi/src/c_api.rs`, and
// `runtimes/core`'s `#[repr(C)]` structs are maintained BY HAND and must mirror
// each other's field order/sizes exactly. A silently reordered field is memory
// corruption. This translation unit `static_assert`s those invariants so any
// drift fails the build.
//
// Every offset is spelled in terms of the target's own pointer size and u64
// member alignment, because a stock React Native Android build compiles
// armeabi-v7a and x86 alongside the 64-bit ABIs.
//
// The Rust side pins the same facts in `c_api.rs`'s `#[cfg(test)] mod tests`.
#include "ubrn_jsi.h"

#include <cstddef>
#include <cstdint>

namespace {

// The spec structs mix pointers and `size_t`, so pin the two to one width here
// and spell every offset below in pointers.
constexpr std::size_t kPtr = sizeof(void *);
static_assert(sizeof(std::size_t) == kPtr, "size_t must be pointer-sized");

constexpr std::size_t kU64 = sizeof(std::uint64_t);

// The alignment a u64 *member* imposes on its enclosing struct. Measured
// through a struct, not as `alignof(std::uint64_t)`: on every i386 target,
// a member forces only 4 while that spelling reports clang's preferred
// alignment of 8. It is what makes UbrnRustBuffer 20 bytes on x86 and
// 24 on armeabi-v7a.
struct U64Member {
  std::uint64_t value;
};
constexpr std::size_t kU64Align = alignof(U64Member);

// `n` rounded up to the next multiple of `a`.
constexpr std::size_t round_up(std::size_t n, std::size_t a) {
  return (n + a - 1) / a * a;
}

} // namespace

// --- UbrnRustBuffer must match core::RustBufferC (ffi_c_types.rs) ------------
// `{ u64 capacity; u64 len; *mut u8 data }`. The two u64s put `data` at 16,
// which is already pointer-aligned everywhere; the tail pads out to the
// alignment a u64 member imposes: 24 bytes on 64-bit and armeabi-v7a, 20 on
// x86.
static_assert(offsetof(UbrnRustBuffer, capacity) == 0,
              "RustBuffer.capacity is first");
static_assert(offsetof(UbrnRustBuffer, len) == kU64,
              "RustBuffer.len follows capacity");
static_assert(offsetof(UbrnRustBuffer, data) == 2 * kU64,
              "RustBuffer.data follows len");
static_assert(sizeof(UbrnRustBuffer) == round_up(2 * kU64 + kPtr, kU64Align),
              "RustBuffer is two u64s plus a pointer, padded to u64 alignment");

// --- Spec struct layouts must match c_api.rs's #[repr(C)] mirrors ------------
// These structs are passed by pointer from JS-built native memory into Rust; a
// field reorder on either side corrupts every registration. Every member is
// pointer-sized bar the trailing `u8` flags, so the offsets are counts of
// pointers.

// UbrnFunctionSpec { *name, *arg_tag_names, size_t n_args, *arg_type_names,
//                    *ret_tag_name, u8 has_rust_call_status }.
static_assert(offsetof(UbrnFunctionSpec, name) == 0, "FunctionSpec.name @0");
static_assert(offsetof(UbrnFunctionSpec, arg_tag_names) == kPtr,
              "FunctionSpec.arg_tag_names @1 pointer");
static_assert(offsetof(UbrnFunctionSpec, n_args) == 2 * kPtr,
              "FunctionSpec.n_args @2 pointers");
static_assert(offsetof(UbrnFunctionSpec, arg_type_names) == 3 * kPtr,
              "FunctionSpec.arg_type_names @3 pointers");
static_assert(offsetof(UbrnFunctionSpec, ret_tag_name) == 4 * kPtr,
              "FunctionSpec.ret_tag_name @4 pointers");
static_assert(offsetof(UbrnFunctionSpec, has_rust_call_status) == 5 * kPtr,
              "FunctionSpec.has_rust_call_status @5 pointers");
// The trailing u8 pads out to pointer alignment.
static_assert(sizeof(UbrnFunctionSpec) == 6 * kPtr, "FunctionSpec size");

// UbrnCallbackSpec { *name, *arg_tag_names, *arg_type_names, size_t n_args,
//                    *ret_tag_name, u8 has_rust_call_status, u8 out_return,
//                    *ret_type_name }.
static_assert(offsetof(UbrnCallbackSpec, name) == 0, "CallbackSpec.name @0");
static_assert(offsetof(UbrnCallbackSpec, arg_tag_names) == kPtr,
              "CallbackSpec.arg_tag_names @1 pointer");
static_assert(offsetof(UbrnCallbackSpec, arg_type_names) == 2 * kPtr,
              "CallbackSpec.arg_type_names @2 pointers");
static_assert(offsetof(UbrnCallbackSpec, n_args) == 3 * kPtr,
              "CallbackSpec.n_args @3 pointers");
static_assert(offsetof(UbrnCallbackSpec, ret_tag_name) == 4 * kPtr,
              "CallbackSpec.ret_tag_name @4 pointers");
static_assert(offsetof(UbrnCallbackSpec, has_rust_call_status) == 5 * kPtr,
              "CallbackSpec.has_rust_call_status @5 pointers");
// The two u8 flags share one pointer-sized slot.
static_assert(offsetof(UbrnCallbackSpec, out_return) == 5 * kPtr + 1,
              "CallbackSpec.out_return immediately follows has_rust_call_status");
static_assert(offsetof(UbrnCallbackSpec, ret_type_name) == 6 * kPtr,
              "CallbackSpec.ret_type_name @6 pointers");
static_assert(sizeof(UbrnCallbackSpec) == 7 * kPtr, "CallbackSpec size");

// UbrnStructField { *field_name, *type_tag_name, *type_name }.
static_assert(offsetof(UbrnStructField, field_name) == 0,
              "StructField.field_name @0");
static_assert(offsetof(UbrnStructField, type_tag_name) == kPtr,
              "StructField.type_tag_name @1 pointer");
static_assert(offsetof(UbrnStructField, type_name) == 2 * kPtr,
              "StructField.type_name @2 pointers");
static_assert(sizeof(UbrnStructField) == 3 * kPtr, "StructField size");

// UbrnStructSpec { *name, *fields, size_t n_fields }.
static_assert(offsetof(UbrnStructSpec, name) == 0, "StructSpec.name @0");
static_assert(offsetof(UbrnStructSpec, fields) == kPtr,
              "StructSpec.fields @1 pointer");
static_assert(offsetof(UbrnStructSpec, n_fields) == 2 * kPtr,
              "StructSpec.n_fields @2 pointers");
static_assert(sizeof(UbrnStructSpec) == 3 * kPtr, "StructSpec size");

// UbrnModuleSpec { *rustbuffer_alloc, *rustbuffer_free, *rustbuffer_from_bytes,
//                  *functions, size_t n_functions, *callbacks, size_t
//                  n_callbacks, *structs, size_t n_structs }.
static_assert(offsetof(UbrnModuleSpec, rustbuffer_alloc) == 0,
              "ModuleSpec.rustbuffer_alloc @0");
static_assert(offsetof(UbrnModuleSpec, rustbuffer_free) == kPtr,
              "ModuleSpec.rustbuffer_free @1 pointer");
static_assert(offsetof(UbrnModuleSpec, rustbuffer_from_bytes) == 2 * kPtr,
              "ModuleSpec.rustbuffer_from_bytes @2 pointers");
static_assert(offsetof(UbrnModuleSpec, functions) == 3 * kPtr,
              "ModuleSpec.functions @3 pointers");
static_assert(offsetof(UbrnModuleSpec, n_functions) == 4 * kPtr,
              "ModuleSpec.n_functions @4 pointers");
static_assert(offsetof(UbrnModuleSpec, callbacks) == 5 * kPtr,
              "ModuleSpec.callbacks @5 pointers");
static_assert(offsetof(UbrnModuleSpec, n_callbacks) == 6 * kPtr,
              "ModuleSpec.n_callbacks @6 pointers");
static_assert(offsetof(UbrnModuleSpec, structs) == 7 * kPtr,
              "ModuleSpec.structs @7 pointers");
static_assert(offsetof(UbrnModuleSpec, n_structs) == 8 * kPtr,
              "ModuleSpec.n_structs @8 pointers");
static_assert(sizeof(UbrnModuleSpec) == 9 * kPtr, "ModuleSpec size");
