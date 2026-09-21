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

// Per-module release valve for workers parked in cb_dispatch's rendezvous.
//
// A runtime torn down with a posted task still queued discards it, so that
// call's done flag is never set. Without this the worker waits forever, holding
// a frame of the loaded library below it. One mutex and condvar per module,
// shared by all its cross-thread callbacks: a per-call pair cannot be reached
// from a destructor that knows only the module.
struct AbortState {
  std::mutex mtx;
  std::condition_variable cv;
  bool aborted = false;
};

// Release every worker parked in this module's rendezvous. Idempotent, and safe
// in a destructor: it touches only the shim's own mutex and condvar, never the
// runtime. A released waiter leaves its return bytes zeroed, which is what
// core's unloading path produces anyway.
//
// Call it only on the thread that drains this module's CallInvoker, and only
// with that runtime about to be destroyed. A released worker returns and pops
// its frame, while a task already posted for it would write the callback's
// result through out_return and RustCallStatus pointers into that same frame.
// The task checks `aborted` before calling into JS and returns without doing
// so, so a queued task that does run after an abort touches nothing; teardown
// discarding the queue is the usual case, not the only defence.
//
// The sole caller is ~UniffiPlayerRoot, and JSI promises neither half of that:
// a host object's dtor runs on an unspecified thread and may be as late as
// runtime shutdown. What holds it up is reachability. The root is owned by the
// `uniffi` global and by every live module object, so nothing collects it while
// the runtime those belong to is still serving calls. The caller checks the
// thread half at runtime rather than trusting it: the shim builds with NDEBUG,
// so its assert is documentation only.
void abortModule(const ModuleCallbackInfo &info);

// Mirror of core::RustCallStatusC (repr(C), natural alignment — do NOT pack).
// Because `code` is first, a `*mut RustCallStatus` cast to this is layout-
// compatible (see NAPI RustCallStatusForVTable). error_buf's layout matches
// UbrnRustBuffer ({u64 capacity; u64 len; u8* data}); its size is target-
// dependent (pointer width). Shared by both the call path (shim.cpp) and the
// Rust->JS vtable path (callbacks.cpp).
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
  std::string retName;   // for RustBuffer/scalar returns retName is empty
  size_t retTagSize = 0; // core's slot width for retTag, outReturn or not
  size_t retSize = 0;    // 0 for void or outReturn, else retTagSize
  // Total byte length of the full arg buffer core's trampoline passes:
  // [declared_args, out_return_ptr?, RCS_ptr?]. Mirrors core's
  // ArgLayout::total_size. Used by the cross-thread copy in cb_dispatch.
  size_t totalSize = 0;
  // False when core gave no flat arg layout for this callback, leaving
  // argSlots/outReturnSlot/rcsSlot/totalSize unset. The trampoline fns must
  // refuse such a shape instead of indexing into the empty slots.
  bool slotsValid = false;
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
  // Borrowed module callback/struct registry, needed on the JS thread to
  // marshal Struct args/returns and wrap incoming completer fn ptrs.
  //
  // Every owner of it is a JS object of one runtime — the module object's
  // host-function closures and the player root — so it dies with that runtime
  // while this leaked userdata does not. What keeps the back-pointer safe is
  // that it is read only from cb_on_js_thread, on the JS thread, and core
  // reaches that only through a trampoline the unloading flag has not
  // disarmed. Teardown disarms before the registry goes.
  const ModuleCallbackInfo *info = nullptr;
  // The invoker and JS thread of the runtime this trampoline was built against.
  // Per-trampoline, not process-global: a reload builds a second runtime, and a
  // global would leave this trampoline posting the first runtime's
  // jsi::Function onto the second runtime's invoker.
  //
  // The shared_ptr keeps the invoker object alive past its runtime, so the
  // dereference in cb_dispatch is never a dangling one. It says nothing about
  // the scheduler behind it: a task posted onto a dead runtime's invoker is
  // simply never drained, which is what the abort exists to survive.
  std::shared_ptr<facebook::react::CallInvoker> callInvoker;
  std::thread::id jsThreadId;
  // Shared with every other cross-thread callback of this module; see
  // AbortState. Copied here so the worker-thread path never dereferences
  // `info`.
  std::shared_ptr<AbortState> abortState;
};

// Assemble a callback's CallbackShape, reading the slot offsets/sizes from
// core (ubrn_jsi_callback_arg_layout) rather than deriving them here. `module`
// must already be registered, since core keys the layout by callback name.
// A callback core has no flat arg buffer for (a struct passed by value, as in
// ForeignFutureComplete*) gets a shape with no slots: core declines to build a
// trampoline for it too, so only the fn-pointer path — which reads `args` —
// ever sees it.
CallbackShape buildShape(jsi::Runtime &rt, UbrnJsiModule *module,
                         const std::string &cbName,
                         const std::vector<ArgDesc> &args, const ArgDesc &ret,
                         bool hasRcs, bool outReturn);

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
// kept alive for as long as the runtime that registered them: the owners are
// the module object's closures and the player root, all JS objects of that one
// runtime.
struct ModuleCallbackInfo {
  // The registered module these layouts describe, so the runtime-scoped root
  // can disarm it at teardown. Borrowed: core owns it and it is never freed.
  UbrnJsiModule *module = nullptr;

  // Copied off the runtime-scoped root at registration and handed to every
  // trampoline this module builds; see CbUserData for why they are not global.
  std::shared_ptr<facebook::react::CallInvoker> callInvoker;
  std::thread::id jsThreadId;

  // Created with the module and shared by every trampoline it builds, so the
  // root can release all of them knowing only the module.
  std::shared_ptr<AbortState> abortState = std::make_shared<AbortState>();

  // The invoker and its thread id are only meaningful as a pair: an invoker set
  // beside a default-constructed thread id answers `false` for a JS-thread
  // callback, which routes it into cb_dispatch and blocks the JS thread on a
  // task only the JS thread can drain. Bound in one call so no site can set one
  // and forget the other.
  void bindRuntime(std::shared_ptr<facebook::react::CallInvoker> invoker,
                   std::thread::id thread) {
    callInvoker = std::move(invoker);
    jsThreadId = thread;
  }

  std::map<std::string, CallbackShape> callbacks; // callback name -> shape
  std::map<std::string, StructLayout>
      structs; // struct name -> Callback field list
  std::map<std::string, StructDesc>
      structDescs; // struct name -> all-field descs

  // Backs the $uniffiTrampolineCount diagnostic — the only way JS can see the
  // per-call trampoline leak that reuse exists to prevent. Trampolines
  // themselves live in core's map; see remember_trampoline for why reuse is
  // correct. Mutable because it is bumped through a const info&; written only
  // on the JS thread, the only thread that marshals a Callback-typed value.
  mutable uint64_t trampolinesBuilt = 0;
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
