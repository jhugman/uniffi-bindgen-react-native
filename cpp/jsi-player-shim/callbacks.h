/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// callbacks.h — Rust -> JS callback machinery for the JSI player.
//
// Ports runtimes/napi/src/callback/{mod,vtable}.rs to C++/JSI. The NAPI
// ThreadsafeFunction becomes React Native's CallInvoker; the napi sync_channel
// rendezvous becomes a std::mutex + condition_variable.
//
// The three extern "C" fns (cb_on_js_thread / cb_dispatch / cb_is_js_thread)
// implement core's trampoline protocol: core packs the libffi args into a flat
// byte buffer laid out as [declared_args, out_return_ptr?, RCS_ptr?], then
// either calls on_js_thread directly (same-thread) or hands it to dispatch
// (worker thread). We read the buffer per the callback's CallbackShape, call
// the JS method, and write the return bytes back.
#pragma once
#include <ReactCommon/CallInvoker.h>
#include <jsi/jsi.h>

#include <condition_variable>
#include <cstdint>
#include <cstring>
#include <map>
#include <memory>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

#include "ubrn_jsi.h"
#include "value_conv.h"

namespace jsi = facebook::jsi;

namespace ubrn_cb {

// Forward declaration: CbUserData holds a back-pointer to the registry.
struct ModuleCallbackInfo;

// Host CallInvoker + JS thread id, captured in registerNatives. Cross-thread
// callbacks post onto the invoker; same-thread ones run directly.
extern std::shared_ptr<facebook::react::CallInvoker> g_callInvoker;
extern std::thread::id g_jsThreadId;

// Mirror of core::RustCallStatusC (repr(C), natural alignment — do NOT pack).
// Because `code` is first, a `*mut RustCallStatus` cast to this is layout-
// compatible (see NAPI RustCallStatusForVTable). The 24-byte error_buf matches
// UbrnRustBuffer ({u64 capacity; u64 len; u8* data}). Shared by both the call
// path (shim.cpp) and the Rust->JS vtable path (callbacks.cpp).
struct RustCallStatus {
  int8_t code;
  UbrnRustBuffer error_buf;
};

// One slot in a callback's flat arg buffer: byte offset + size.
struct SlotLayout {
  size_t offset;
  size_t size;
};

// Precomputed layout/shape of one callback, mirroring core's ArgLayout +
// CallbackDef. Built once at vtable-construction time, read on every
// invocation.
struct CallbackShape {
  std::vector<ArgDesc> args;        // declared positional args
  std::vector<SlotLayout> argSlots; // offsets for declared args only
  bool hasRcs = false;
  bool outReturn = false;
  SlotLayout rcsSlot{0, 0};       // valid iff hasRcs
  SlotLayout outReturnSlot{0, 0}; // valid iff outReturn (the extra ptr arg)
  uint8_t retTag = UBRN_TY_VOID;
  std::string retName; // for RustBuffer/scalar returns retName is empty
  size_t retSize = 0;  // 0 for void or outReturn
  // Total byte length of the full arg buffer core's trampoline passes:
  // [declared_args, out_return_ptr?, RCS_ptr?]. Mirrors core's
  // ArgLayout::total_size. Used by the cross-thread copy in cb_dispatch.
  size_t totalSize = 0;
};

// Per-callback userdata passed to core's trampoline as `user_data`. Heap-
// allocated and LEAKED (process lifetime): the Rust library may invoke the
// vtable from any thread at any future time. The jsi::Function is only touched
// on the JS thread.
struct CbUserData {
  jsi::Runtime *rt;
  std::shared_ptr<jsi::Function> jsFn;
  CallbackShape shape;
  UbrnJsiModule *module;
  // Borrowed (process-lifetime) module callback/struct registry, needed on the
  // JS thread to marshal Struct args/returns and wrap incoming completer fn
  // ptrs. Owned by the module object's closure (a shared_ptr kept alive for the
  // process lifetime); CbUserData is itself leaked, so this back-pointer is
  // safe.
  const ModuleCallbackInfo *info = nullptr;
};

// Compute a callback's CallbackShape from its parsed ArgDescs. Reproduces
// core::ArgLayout::compute: natural alignment, declared args first, then (if
// out_return) a pointer slot, then (if hasRcs) the RustCallStatus pointer slot.
CallbackShape buildShape(const std::vector<ArgDesc> &args, uint8_t retTag,
                         const std::string &retName, bool hasRcs,
                         bool outReturn);

// The three trampoline fns (signatures match the ubrn_jsi.h typedefs).
extern "C" void cb_on_js_thread(const uint8_t *args, uint8_t *ret,
                                const void *ud);
extern "C" void cb_dispatch(UbrnOnJsThreadFn on_js, const uint8_t *args,
                            uint8_t *ret, const void *ud);
extern "C" bool cb_is_js_thread(const void *ud);

// One vtable struct's field list: ordered (jsMethodName, callbackName) pairs.
// Only Callback-typed fields are recorded here (the vtable builder maps a JS
// method name to a trampoline). For struct-by-value marshalling, the full field
// type info is kept separately in `StructDesc` below.
struct StructLayout {
  std::vector<std::pair<std::string, std::string>>
      fields; // (field/method name, callback name)
};

// Full description of a struct's fields (name + ArgDesc), for marshalling a JS
// object to/from the struct's C bytes. Mirrors core's StructDef; field byte
// offsets/sizes come from ubrn_jsi_struct_field_offsets (libffi inside core).
struct StructFieldDesc {
  std::string name;
  ArgDesc type;
};
struct StructDesc {
  std::vector<StructFieldDesc> fields;
};

// Cached per-module callback + struct layouts, parsed once at register time and
// kept alive for the process lifetime (captured by the module object closures).
struct ModuleCallbackInfo {
  std::map<std::string, CallbackShape> callbacks; // callback name -> shape
  std::map<std::string, StructLayout>
      structs; // struct name -> Callback field list
  std::map<std::string, StructDesc>
      structDescs; // struct name -> all-field descs
};

// Build a trampoline for a Callback-typed value (a JS function), returning the
// fn pointer (leaked, process lifetime). Shared by ALL three Callback-marshal
// sites: struct-field marshalling (marshalFieldToBytes), the per-vtable-field
// loop (buildVTableStruct), and the plain Callback fn-ptr function-arg arm in
// shim.cpp. Mirrors the Callback arm of marshal_field_to_bytes.
//
// `cbName` names the callback shape to look up in `info.callbacks` (and is the
// trampoline's debug name). `v` is the JS value that must be a function — at
// the vtable-field site this comes from a DIFFERENT property (the JS method
// name) than the callback name, so it is passed separately. `errLabel` is the
// human-readable name used in the "is not a function" error (a struct field
// name, a JS method name, or a host-function name) so each site keeps a
// descriptive message.
const void *trampolineForJsFn(jsi::Runtime &rt, UbrnJsiModule *module,
                              const ModuleCallbackInfo &info,
                              const std::string &cbName, const jsi::Value &v,
                              const std::string &errLabel);

// Build a vtable for `structName` from a JS object whose properties are the
// methods named by the struct's fields. For each field (a Callback(name)):
// grab the JS method, build a leaked CbUserData, make a trampoline, then call
// ubrn_jsi_build_vtable. Returns the vtable pointer (also leaked, by core).
// Ports vtable.rs::build_vtable_struct.
const void *buildVTableStruct(jsi::Runtime &rt, UbrnJsiModule *module,
                              const ModuleCallbackInfo &info,
                              const std::string &structName,
                              const jsi::Object &jsObj);

// Marshal a JS object into the C bytes of struct `structName`, matching the
// platform's C struct layout (offsets/sizes from
// ubrn_jsi_struct_field_offsets). Ports marshal_js_struct_to_bytes:
// scalar/RustBuffer/nested-struct/RustCallStatus/ Callback fields are each
// written at the field's byte offset. Callback fields build a leaked trampoline
// (process lifetime) and store its fn pointer.
std::vector<uint8_t> marshalJsStructToBytes(jsi::Runtime &rt,
                                            UbrnJsiModule *module,
                                            const ModuleCallbackInfo &info,
                                            const std::string &structName,
                                            const jsi::Object &jsObj);

} // namespace ubrn_cb
