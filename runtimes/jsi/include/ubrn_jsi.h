/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

// runtimes/jsi/include/ubrn_jsi.h  (authoritative; mirrored by c_api.rs)
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct UbrnJsiModule UbrnJsiModule;

// Type tags. Values MUST match c_api.rs::ffi_type_from_tag and core::FfiTypeDesc.
typedef enum {
  UBRN_TY_VOID = 0,
  UBRN_TY_U8 = 1,  UBRN_TY_I8 = 2,
  UBRN_TY_U16 = 3, UBRN_TY_I16 = 4,
  UBRN_TY_U32 = 5, UBRN_TY_I32 = 6,
  UBRN_TY_U64 = 7, UBRN_TY_I64 = 8,
  UBRN_TY_F32 = 9, UBRN_TY_F64 = 10,
  UBRN_TY_HANDLE = 11,
  UBRN_TY_RUSTBUFFER = 12, // fully supported (args and return values)
  UBRN_TY_CALLBACK = 13,   // named; the name is in the parallel *_type_names array
  UBRN_TY_STRUCT = 14,     // named; the name is in the parallel *_type_names array
  UBRN_TY_REFERENCE = 15,  // pointer to a named struct (vtable); name in the parallel
                           // *_type_names array; maps to Reference(Struct(name))
  UBRN_TY_RUSTCALLSTATUS = 16, // inline RustCallStatus struct ({i8, u64, u64, ptr});
                               // appears only as a struct field (e.g. inside
                               // ForeignFutureResult<T>); maps to FfiTypeDesc::RustCallStatus
} UbrnFfiType;

typedef struct {
  const char* name;        // raw FFI symbol, e.g. "uniffi_arithmetical_fn_func_add"
  const uint8_t* arg_tags; // array of UbrnFfiType, length n_args
  size_t n_args;
  // Parallel to arg_tags (length n_args): the type name for Callback/Struct tags,
  // NULL for scalar args. The whole array may be NULL (all names absent).
  // Appended AFTER n_args; mirrored at the same offset in c_api.rs.
  const char* const* arg_type_names;
  uint8_t ret_tag;         // UbrnFfiType; scalar/RustBuffer only — named types (Callback/Struct) are not supported as returns
  uint8_t has_rust_call_status; // 0 or 1
} UbrnFunctionSpec;

// One method signature of a callback interface that JS must implement.
typedef struct {
  const char* name;
  const uint8_t* arg_tags;                 // array of UbrnFfiType, length n_args
  const char* const* arg_type_names;       // parallel to arg_tags; name for Callback/Struct tags, NULL for scalars (NULL whole-array allowed)
  size_t n_args;
  uint8_t ret_tag;                         // scalar/RustBuffer for direct returns; for an
                                           // out_return callback this MAY be Struct (the struct
                                           // is written through the out_return pointer)
  uint8_t has_rust_call_status;            // 0 or 1
  uint8_t out_return;                      // 0 or 1
  // The return type's name for a Struct (tag 14) return, else NULL. Carries the
  // struct name so the callback's `ret` parses to Struct(name) (core ignores it
  // for out_return). Appended AFTER out_return; mirrored in c_api.rs.
  const char* ret_type_name;
} UbrnCallbackSpec;

// One field of a vtable struct.
typedef struct {
  const char* field_name;
  uint8_t type_tag;                        // typically UBRN_TY_CALLBACK
  const char* type_name;                   // callback name for UBRN_TY_CALLBACK fields, else NULL
} UbrnStructField;

// A vtable struct definition (e.g. a callback interface's vtable layout).
typedef struct {
  const char* name;
  const UbrnStructField* fields;
  size_t n_fields;
} UbrnStructSpec;

typedef struct {
  const char* rustbuffer_alloc;
  const char* rustbuffer_free;
  const char* rustbuffer_from_bytes;
  const UbrnFunctionSpec* functions;
  size_t n_functions;
  const UbrnCallbackSpec* callbacks;
  size_t n_callbacks;
  const UbrnStructSpec* structs;
  size_t n_structs;
} UbrnModuleSpec;

// dlopen `lib_path`, resolve symbols, build CIFs. Returns NULL on error and writes
// a message into err_buf (NUL-terminated, truncated to err_len).
UbrnJsiModule* ubrn_jsi_register(const char* lib_path,
                                 const UbrnModuleSpec* spec,
                                 char* err_buf, size_t err_len);

// Invoke `fn_name`. `args[i]` points to `arg_sizes[i]` native-endian bytes for arg i
// (scalars, RustBuffer, callback fn-ptrs, and struct/vtable pointers). `status` is a
// *mut RustCallStatus (24+ bytes, caller-allocated) or NULL. The native return value
// is written into `out_ret` (out_ret_size bytes). Returns 0 on success, non-zero on
// engine error.
int ubrn_jsi_call(UbrnJsiModule* m, const char* fn_name,
                  const void* const* args, const size_t* arg_sizes, size_t n_args,
                  void* status, void* out_ret, size_t out_ret_size);

// RustBuffer mirror of core::RustBufferC. MUST match field order/sizes exactly:
// capacity (u64), len (u64), data (*mut u8).
typedef struct {
  uint64_t capacity;
  uint64_t len;
  uint8_t* data;
} UbrnRustBuffer;

UbrnRustBuffer ubrn_jsi_rustbuffer_alloc(UbrnJsiModule* m, uint64_t size);
UbrnRustBuffer ubrn_jsi_rustbuffer_from_bytes(UbrnJsiModule* m, const uint8_t* data, size_t len);
void ubrn_jsi_rustbuffer_free(UbrnJsiModule* m, UbrnRustBuffer buf);

void ubrn_jsi_free(UbrnJsiModule* m);

// --- Callback support (Rust <-> JS) -----------------------------------------

typedef void (*UbrnOnJsThreadFn)(const uint8_t* args, uint8_t* ret, const void* user_data);
typedef void (*UbrnDispatchFn)(UbrnOnJsThreadFn on_js_thread, const uint8_t* args,
                               uint8_t* ret, const void* user_data);
typedef bool (*UbrnIsJsThreadFn)(const void* user_data);

// Create a libffi trampoline the loaded Rust library can invoke to call into JS.
// Returns NULL on error (unknown callback name / bad module).
const void* ubrn_jsi_make_trampoline(UbrnJsiModule* m, const char* callback_name,
                                     UbrnOnJsThreadFn on_js_thread, UbrnDispatchFn dispatch,
                                     UbrnIsJsThreadFn is_js_thread, const void* user_data);

// Build a vtable byte blob from ordered (callback_name, fn_ptr) pairs. Returns NULL on error.
const void* ubrn_jsi_build_vtable(UbrnJsiModule* m, const char* struct_name,
                                  const char* const* callback_names,
                                  const void* const* fn_ptrs, size_t n);

// Invoke a fn pointer (e.g. a future completer) using the named callback's signature.
// arg_blob is the concatenation of each arg's repr(C) bytes; sizes[i] splits it. Returns 0 on success.
int ubrn_jsi_call_callback_ptr(UbrnJsiModule* m, const char* callback_name, const void* fn_ptr,
                               const uint8_t* arg_blob, const size_t* sizes, size_t n_args);

// Query the C struct layout (computed via libffi inside core) for a registered struct.
// Writes the total struct size to *out_total_size and, for up to `cap` fields, the
// per-field byte offset/size into out_offsets[i]/out_sizes[i]. Returns the struct's
// real field count (>= cap means out_offsets/out_sizes were truncated; call again with
// a larger cap), or -1 on error (null module, unknown struct, null name).
// Mirrors core::Module::struct_field_offsets.
int ubrn_jsi_struct_field_offsets(UbrnJsiModule* m, const char* struct_name,
                                  size_t* out_total_size, size_t* out_offsets,
                                  size_t* out_sizes, size_t cap);

#ifdef __cplusplus
} // extern "C"
#endif
