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

typedef struct {
  const char* name;        // raw FFI symbol, e.g. "uniffi_arithmetical_fn_func_add"
  // Player tag names, length n_args, e.g. "UInt8" / "Callback". NUL-terminated,
  // valid for the ubrn_jsi_register call.
  const char* const* arg_tag_names;
  size_t n_args;
  // Parallel to arg_tag_names (length n_args): the type name for Callback,
  // Struct and Reference tags, NULL for scalars. The whole array may be NULL.
  const char* const* arg_type_names;
  const char* ret_tag_name;     // player tag name; scalar/RustBuffer only
  uint8_t has_rust_call_status; // 0 or 1
} UbrnFunctionSpec;

// One method signature of a callback interface that JS must implement.
typedef struct {
  const char* name;
  // Player tag names, length n_args. NUL-terminated, valid for the
  // ubrn_jsi_register call.
  const char* const* arg_tag_names;
  const char* const* arg_type_names;       // parallel to arg_tag_names; name for Callback/Struct/Reference tags, NULL for scalars (NULL whole-array allowed)
  size_t n_args;
  const char* ret_tag_name;                // player tag name; scalar/RustBuffer for direct
                                           // returns; for an out_return callback this MAY be
                                           // "Struct" (the struct is written through the
                                           // out_return pointer)
  uint8_t has_rust_call_status;            // 0 or 1
  uint8_t out_return;                      // 0 or 1
  // The return type's name for a "Struct" return, else NULL. Carries the
  // struct name so the callback's `ret` parses to Struct(name) (core ignores it
  // for out_return). Appended AFTER out_return; mirrored in c_api.rs.
  const char* ret_type_name;
} UbrnCallbackSpec;

// One field of a vtable struct.
typedef struct {
  const char* field_name;
  const char* type_tag_name;  // player tag name; typically "Callback"
  const char* type_name;      // callback name for Callback fields, else NULL
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
// *mut RustCallStatus (sizeof(RustCallStatus) bytes, caller-allocated) or NULL. The
// native return value is written into `out_ret` (out_ret_size bytes). Returns 0 on
// success, non-zero on engine error.
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

// Stop this module serving its frontend: set the unloading flag and invoke the
// abort hook registered at construction. Idempotent, and tolerates a NULL handle.
// Frees nothing, drains nothing and does not close the library — the module stays
// valid to call, but every trampoline it handed out now returns without calling
// into the frontend, zeroing whatever return bytes it has. An out_return callback
// has none, so Rust reads back FfiDefault::ffi_default() with call_status.code
// still 0. No new trampoline or vtable can be built.
// Mirrors core::Module::disarm.
void ubrn_jsi_disarm(UbrnJsiModule* m);

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

// The trampoline already built for (callback_name, identity), or NULL if none.
// `identity` is a caller-minted number naming one JS function. Core owns the reuse map,
// the caller owns the naming, since JS identity is not something core can observe — so
// keeping identities distinct is the caller's job, and across every module it registers
// rather than one: a JS function reaching two modules carries one number into both.
// Also NULL on error (null module, null/not-UTF-8 name) — a miss and an error mean the
// same thing to a caller: build one.
// Mirrors core::Module::trampoline_for.
const void* ubrn_jsi_trampoline_for(UbrnJsiModule* m, const char* callback_name,
                                    uint64_t identity);

// Record fn_ptr as the trampoline for (callback_name, identity), so a later call with
// the same pair reuses it instead of building — and leaking — another. A null fn_ptr is
// recorded as-is; it would read back as a miss, costing reuse, not correctness.
// Mirrors core::Module::remember_trampoline.
void ubrn_jsi_remember_trampoline(UbrnJsiModule* m, const char* callback_name,
                                  uint64_t identity, const void* fn_ptr);

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
// real field count (> cap means out_offsets/out_sizes were truncated; call again with
// a larger cap), or -1 on error (null module, unknown struct, null name).
// Mirrors core::Module::struct_field_offsets.
int ubrn_jsi_struct_field_offsets(UbrnJsiModule* m, const char* struct_name,
                                  size_t* out_total_size, size_t* out_offsets,
                                  size_t* out_sizes, size_t cap);

// Byte size and alignment of one flat argument slot for a player tag name.
// Either out-param may be NULL. Pure function of the tag name, resolved at
// registration. Returns false for a name with no slot geometry.
bool ubrn_jsi_scalar_slot_size_align(const char* tag_name, size_t* out_size, size_t* out_align);

// Byte width of a function's return value as ubrn_jsi_call writes it, for a
// player tag name: the slot geometry above, except that a pointer return is
// always 8 bytes. Size a function's out_ret from this, never from the slot
// geometry, which is pointer-width and so too small on a 32-bit host.
// out_size may be NULL. Returns false for a name with no return width
// (an unrecognized tag, or a Struct).
bool ubrn_jsi_return_size(const char* tag_name, size_t* out_size);

// Byte offsets and sizes of a callback's argument slots, in CIF order
// [declared args, out-return ptr?, RustCallStatus ptr?] — the layout core's
// trampoline packs. Writes the whole buffer's byte length to *out_total_size
// and, for up to `cap` slots, the per-slot offset/size into
// out_offsets[i]/out_sizes[i]. Returns the callback's real slot count (> cap
// means out_offsets/out_sizes were truncated; call again with a larger cap),
// or -1 on error (null module, unknown callback, null name).
// Mirrors core::Module::callback_arg_layout.
int ubrn_jsi_callback_arg_layout(UbrnJsiModule* m, const char* callback_name,
                                 size_t* out_total_size, size_t* out_offsets,
                                 size_t* out_sizes, size_t cap);

#ifdef __cplusplus
} // extern "C"
#endif
