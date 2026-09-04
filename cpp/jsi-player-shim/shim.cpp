/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#include <ReactCommon/CallInvoker.h>
#include <jsi/jsi.h>

#include <atomic>
#include <cassert>
#include <cstdint>
#include <cstring>
#include <deque>
#include <memory>
#include <string>
#include <thread>
#include <vector>

#include "callbacks.h" // Rust -> JS callback machinery (vtable building, the three fns)
#include "ubrn_jsi.h"   // from runtimes/jsi/include
#include "value_conv.h" // ArgDesc / scalarToBytes / bytesToScalar / arrayBytes

namespace jsi = facebook::jsi;

namespace {

// The C++ mirror of core::RustCallStatusC lives in callbacks.h
// (ubrn_cb::RustCallStatus) and is shared with the Rust->JS vtable path.
using ubrn_cb::RustCallStatus;

// One registered function: its symbol name, the arg/ret type descs, and whether
// it carries a RustCallStatus. Captured by the JSI host function closure. The
// arg descs carry the struct name for Reference(Struct) (vtable-pointer) args.
struct FnInfo {
  std::string name;
  std::vector<ArgDesc> args;
  uint8_t retTag;
  // Core's slot width for retTag, resolved at registration (0 for void).
  size_t retSize;
  bool hasRcs;
};

// A jsi buffer that owns the std::string holding its bytes.
//
// Used by string_to_buffer: `jsi::String::utf8()` already returns a string
// owning exactly the UTF-8 bytes, so handing that to JS directly avoids
// allocating a second buffer and copying into it.
class OwnedStringBuffer : public jsi::MutableBuffer {
public:
  explicit OwnedStringBuffer(std::string &&s) : s_(std::move(s)) {}
  size_t size() const override { return s_.size(); }
  uint8_t *data() override { return reinterpret_cast<uint8_t *>(s_.data()); }

private:
  std::string s_;
};

// Build a JS Uint8Array aliasing a returned RustBuffer (freed when GC'd).
//
// ArrayBuffer::size() reports `rb.len` (what string/byte-array lift() decoders
// expect). The destructor frees with the original `rb` (capacity intact), so no
// capacity hint is needed on the return path; the alloc path hands out a view
// whose `len` is set to the capacity so lower() can fill the whole region.
jsi::Value rustBufferToUint8Array(jsi::Runtime &rt, UbrnJsiModule *m,
                                  UbrnRustBuffer rb) {
  if (rb.len == 0 || rb.data == nullptr) {
    // Free any empty-but-allocated buffer before returning a fresh JS array,
    // since nothing will alias it.
    if (rb.data != nullptr)
      ubrn_jsi_rustbuffer_free(m, rb);
    auto ctor = rt.global().getPropertyAsFunction(rt, "Uint8Array");
    return ctor.callAsConstructor(rt, 0);
  }
  auto buf = std::make_shared<RustOwnedBuffer>(m, rb);
  auto ab = jsi::ArrayBuffer(rt, buf);
  // Tag the ArrayBuffer so that if this view comes back as an FFI argument the
  // allocation is adopted rather than copied — see rustBufferForArg.
  ab.setNativeState(rt, std::make_shared<RustBufferOwner>(buf));
  auto ctor = rt.global().getPropertyAsFunction(rt, "Uint8Array");
  return ctor.callAsConstructor(rt, ab);
}

// Functions whose signature contains a still-unsupported tag are skipped at
// registration time rather than rejecting the whole module. Per
// value_conv.h's argDescFromDefObject, the remaining UBRN_TY_UNSUPPORTED
// triggers are a Reference/MutReference to a non-Struct inner type, or any
// unrecognized tag. A skipped function is simply not installed on the native
// module; calling one surfaces a "not a function" error at the JS call site,
// acceptable while out of scope. Reference(Struct) (vtable) args and plain
// Callback fn-ptr args ARE supported here (the `callbacks` fixture).
//
// Note: ArgDesc, argDescFromDefObject, and UBRN_TY_UNSUPPORTED all live in
// value_conv.h (shared with the callback machinery).

// The `uniffi` global, defined below.
class UniffiPlayerRoot;

// Build the native module object from a parsed DEFINITIONS and a registered
// handle.
//
// `root` is captured, not used: a module object that JS can still call keeps
// alive the root that disarms it, so reassigning `globalThis.uniffi` cannot
// collect the root out from under a live module.
jsi::Value
buildModuleObject(jsi::Runtime &rt, UbrnJsiModule *handle,
                  std::shared_ptr<std::vector<FnInfo>> fns,
                  std::shared_ptr<ubrn_cb::ModuleCallbackInfo> cbInfo,
                  std::shared_ptr<UniffiPlayerRoot> root) {
  jsi::Object mod(rt);
  for (const auto &fn : *fns) {
    FnInfo info = fn; // copy into the closure
    auto hostFn = jsi::Function::createFromHostFunction(
        rt, jsi::PropNameID::forUtf8(rt, info.name),
        (unsigned)info.args.size() + (info.hasRcs ? 1 : 0),
        [handle, info, cbInfo, root](jsi::Runtime &rt, const jsi::Value &,
                                     const jsi::Value *args,
                                     size_t count) -> jsi::Value {
          size_t nDeclared = info.args.size();

          // Guard the lower bound before indexing args[i] below: a JS caller
          // passing fewer args than declared would otherwise read past the end
          // of the args array and marshal garbage into the FFI call.
          if (count < nDeclared) {
            throw jsi::JSError(rt, "uniffi jsi player: " + info.name +
                                       " expects " + std::to_string(nDeclared) +
                                       " args, got " + std::to_string(count));
          }

          // Marshal args into contiguous backing storage. Scalars are written
          // as native bytes; a RustBuffer arg is a JS Uint8Array that we copy
          // into a Rust-owned buffer (via rustbuffer_from_bytes) whose repr(C)
          // layout (size target-dependent) is then stored as the arg payload.
          std::vector<std::vector<uint8_t>> backing(nDeclared);
          std::vector<const void *> argPtrs(nDeclared);
          std::vector<size_t> argSizes(nDeclared);
          for (size_t i = 0; i < nDeclared; i++) {
            uint8_t tag = info.args[i].tag;
            size_t sz = info.args[i].size;
            backing[i].resize(sz);
            if (tag == UBRN_TY_RUSTBUFFER) {
              UbrnRustBuffer rb = rustBufferForArg(rt, handle, args[i]);
              memcpy(backing[i].data(), &rb, sizeof(rb));
            } else if (tag == UBRN_TY_REFERENCE) {
              // A vtable-pointer arg: the JS value is a plain object whose
              // properties are the struct's methods. Build the C vtable and
              // store the 8-byte pointer into the arg slot.
              auto jsObj = args[i].asObject(rt);
              const void *vtable = ubrn_cb::buildVTableStruct(
                  rt, handle, *cbInfo, info.args[i].name, jsObj);
              memcpy(backing[i].data(), &vtable, sizeof(vtable));
            } else if (tag == UBRN_TY_CALLBACK) {
              // A plain Callback-typed fn-ptr arg (e.g. the rust_future_poll_*
              // continuation). Single-callback analogue of buildVTableStruct:
              // grab the JS function, build a leaked CbUserData from the parsed
              // CallbackShape, make a trampoline, and store the 8-byte fn ptr.
              // Ports the FfiTypeDesc::Callback arm of napi call/mod.rs. The
              // continuation fires CROSS-THREAD (Rust executor -> cb_dispatch
              // -> invokeAsync), so the userdata is LEAKED for the process
              // lifetime.
              //
              // trampolineForJsFn memoises per (callback name, JS function), so
              // that leak is one per callback type rather than one per call.
              // It has to be: rust_future_poll_* runs once per poll, and the
              // continuation it is handed is a module-level const.
              const std::string &cbName = info.args[i].name;
              const void *fnPtr = ubrn_cb::trampolineForJsFn(
                  rt, handle, *cbInfo, cbName, args[i],
                  info.name + " callback arg '" + cbName + "'");
              memcpy(backing[i].data(), &fnPtr, sizeof(fnPtr));
            } else {
              scalarToBytes(rt, tag, args[i], backing[i].data());
            }
            argPtrs[i] = backing[i].data();
            argSizes[i] = sz;
          }

          RustCallStatus status{};
          void *statusPtr = info.hasRcs ? &status : nullptr;

          // A return is never wider than a RustBuffer, so it fits a stack
          // buffer. This runs on every call, so it must not allocate.
          size_t retSize = info.retSize;
          uint8_t out[sizeof(UbrnRustBuffer)] = {};
          if (retSize > sizeof(out)) {
            throw jsi::JSError(
                rt, "uniffi jsi player: return wider than RustBuffer "
                    "for " +
                        info.name);
          }

          int rc = ubrn_jsi_call(handle, info.name.c_str(), argPtrs.data(),
                                 argSizes.data(), nDeclared, statusPtr, out,
                                 retSize);
          if (rc != 0) {
            throw jsi::JSError(
                rt, "uniffi jsi player: ubrn_jsi_call failed, code " +
                        std::to_string(rc));
          }

          // Write the RustCallStatus code back into the JS status object so the
          // generated uniffiCheckCallStatus can see it. On error (code != 0)
          // with a populated error buffer, copy the error bytes into a fresh JS
          // Uint8Array set as `errorBuf` (the generated lifter decodes it into
          // the right error type), then free the Rust-owned error buffer.
          if (info.hasRcs && count > nDeclared && args[nDeclared].isObject()) {
            auto statusObj = args[nDeclared].getObject(rt);
            statusObj.setProperty(rt, "code", jsi::Value((double)status.code));
            if (status.code != 0 && status.error_buf.data != nullptr &&
                status.error_buf.len > 0) {
              auto ctor = rt.global().getPropertyAsFunction(rt, "Uint8Array");
              auto arr =
                  ctor.callAsConstructor(rt, (double)status.error_buf.len)
                      .asObject(rt);
              auto ab =
                  arr.getPropertyAsObject(rt, "buffer").getArrayBuffer(rt);
              memcpy(ab.data(rt), status.error_buf.data, status.error_buf.len);
              statusObj.setProperty(rt, "errorBuf", arr);
              UbrnRustBuffer err{status.error_buf.capacity,
                                 status.error_buf.len, status.error_buf.data};
              ubrn_jsi_rustbuffer_free(handle, err);
            }
          }

          // A RustBuffer return is the repr(C) UbrnRustBuffer (size target-
          // dependent) written into `out`. Alias it as a Uint8Array whose
          // backing memory is freed when the JS view is GC'd (see
          // RustOwnedBuffer).
          if (info.retTag == UBRN_TY_RUSTBUFFER) {
            UbrnRustBuffer rb;
            memcpy(&rb, out, sizeof(rb));
            return rustBufferToUint8Array(rt, handle, rb);
          }

          return bytesToScalar(rt, info.retTag, out);
        });
    mod.setProperty(rt, info.name.c_str(), hostFn);
  }

  // rustbuffer_alloc(n) -> Uint8Array view over Rust-owned memory of capacity
  // n. The codegen lower() path fills the whole view in place, so the view's
  // `len` is set to the capacity (not 0). Note: the filled view is then copied
  // by `rustbuffer_from_bytes` into a fresh Rust buffer that the FFI call
  // consumes; this alloc'd buffer is itself released separately when its
  // Uint8Array is GC'd. `size` is unchecked here on purpose: the Rust side
  // (c_api.rs) guards `size > i32::MAX` and returns a zeroed buffer, which
  // degrades to an empty array.
  mod.setProperty(
      rt, "rustbuffer_alloc",
      jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forUtf8(rt, "rustbuffer_alloc"), 1,
          [handle](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
                   size_t n) -> jsi::Value {
            if (n < 1) {
              throw jsi::JSError(
                  rt, "uniffi jsi player: rustbuffer_alloc needs a size");
            }
            uint64_t size = (uint64_t)args[0].asNumber();
            UbrnRustBuffer rb = ubrn_jsi_rustbuffer_alloc(handle, size);
            UbrnRustBuffer view = rb;
            view.len = rb.capacity; // expose capacity bytes; lower() fills the
                                    // whole view
            return rustBufferToUint8Array(rt, handle, view);
          }));

  // $uniffiTrampolineCount() -> number of trampolines built for this module.
  //
  // Diagnostic only; codegen never calls it. It exists because the per-call
  // trampoline leak it guards against is invisible from JS: the leaked
  // CbUserData pins the same continuation function object every time, so
  // neither the Hermes heap nor any allocator count moves, and the only
  // symptom is native RSS. Exposing the count lets a fixture assert the
  // invariant directly — one trampoline per callback type, not one per call.
  //
  // The `$` prefix cannot collide with a real entry: every other property here
  // is an FFI symbol name from the module spec.
  mod.setProperty(rt, "$uniffiTrampolineCount",
                  jsi::Function::createFromHostFunction(
                      rt,
                      jsi::PropNameID::forUtf8(rt, "$uniffiTrampolineCount"), 0,
                      [cbInfo](jsi::Runtime &, const jsi::Value &,
                               const jsi::Value *, size_t) -> jsi::Value {
                        return jsi::Value((double)cbInfo->trampolinesBuilt);
                      }));

  // rustbuffer_free(view) — the RustOwnedBuffer destructor frees the underlying
  // allocation on GC, so this is a no-op. Codegen calls it eagerly in a
  // try/finally; freeing here would double-free the still-aliased buffer.
  mod.setProperty(
      rt, "rustbuffer_free",
      jsi::Function::createFromHostFunction(
          rt, jsi::PropNameID::forUtf8(rt, "rustbuffer_free"), 1,
          [](jsi::Runtime &, const jsi::Value &, const jsi::Value *,
             size_t) -> jsi::Value { return jsi::Value::undefined(); }));

  // --- Native string helpers (jsi2 string lift/lower fast path)
  // ---------------
  //
  // The Jsi2 codegen emits the native-helper branch of StringHelperTemplate.ts
  // (supports_text_encoder == false), which routes string conversion through
  // these four module methods instead of an interpreted-JS TextDecoder
  // polyfill. Each uses jsi::String::createFromUtf8 / String::utf8 — the same
  // primitive the gen_cpp JSI path uses (UniffiString.h) — running in native
  // code rather than the Hermes interpreter. The method NAMES must exactly
  // match builders.rs::build_string_helper (no `ubrn_` prefix for the player
  // flavor).

  // ffi__string_from_buffer(bytes, undefined) -> string
  // Decode a whole Uint8Array (the single-string lift fallback, taken when
  // `typeof TextDecoder === "undefined"`).
  mod.setProperty(
      rt, "ubrn_uniffi_internal_fn_func_ffi__string_from_buffer",
      jsi::Function::createFromHostFunction(
          rt,
          jsi::PropNameID::forUtf8(
              rt, "ubrn_uniffi_internal_fn_func_ffi__string_from_buffer"),
          2,
          [](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
             size_t count) -> jsi::Value {
            if (count < 1) {
              throw jsi::JSError(
                  rt, "uniffi jsi player: string_from_buffer needs bytes");
            }
            auto [ptr, len] = arrayBytes(rt, args[0]);
            return jsi::Value(rt, jsi::String::createFromUtf8(rt, ptr, len));
          }));

  // ffi__string_to_buffer(s, undefined) -> Uint8Array
  // Encode a string to its UTF-8 bytes. Returns a real Uint8Array (over a
  // freshly-allocated ArrayBuffer copy of the bytes) so it matches the
  // RustBuffer arg representation the FFI marshalling path (`arrayBytes`)
  // expects on the way back into a Rust call.
  mod.setProperty(
      rt, "ubrn_uniffi_internal_fn_func_ffi__string_to_buffer",
      jsi::Function::createFromHostFunction(
          rt,
          jsi::PropNameID::forUtf8(
              rt, "ubrn_uniffi_internal_fn_func_ffi__string_to_buffer"),
          2,
          [](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
             size_t count) -> jsi::Value {
            if (count < 1) {
              throw jsi::JSError(
                  rt, "uniffi jsi player: string_to_buffer needs a string");
            }
            // Transcode once into a buffer that owns the bytes. The
            // `std::string` from `utf8()` already holds exactly what JS needs,
            // so allocating a second Uint8Array and copying into it is pure
            // overhead on large strings.
            auto payload = std::make_shared<OwnedStringBuffer>(
                args[0].asString(rt).utf8(rt));
            auto ab = jsi::ArrayBuffer(rt, payload);
            auto ctor = rt.global().getPropertyAsFunction(rt, "Uint8Array");
            return ctor.callAsConstructor(rt, ab);
          }));

  // ffi__string_to_byte_length(s, undefined) -> number
  // UTF-8 byte length of a string (used by allocationSize on the write path).
  mod.setProperty(
      rt, "ubrn_uniffi_internal_fn_func_ffi__string_to_byte_length",
      jsi::Function::createFromHostFunction(
          rt,
          jsi::PropNameID::forUtf8(
              rt, "ubrn_uniffi_internal_fn_func_ffi__string_to_byte_length"),
          2,
          [](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
             size_t count) -> jsi::Value {
            if (count < 1) {
              throw jsi::JSError(
                  rt,
                  "uniffi jsi player: string_to_byte_length needs a string");
            }
            return jsi::Value((double)args[0].asString(rt).utf8(rt).size());
          }));

  // ffi__read_string_from_buffer(buf, offset, length) -> string
  // Array-of-strings lift: decode `length` bytes at `offset` directly out of
  // the RustBuffer wrapper's backing ArrayBuffer (`buf.arrayBuffer`), with no
  // intermediate Uint8Array view allocation.
  mod.setProperty(
      rt, "ubrn_uniffi_internal_fn_func_ffi__read_string_from_buffer",
      jsi::Function::createFromHostFunction(
          rt,
          jsi::PropNameID::forUtf8(
              rt, "ubrn_uniffi_internal_fn_func_ffi__read_string_from_buffer"),
          3,
          [](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
             size_t count) -> jsi::Value {
            if (count < 3) {
              throw jsi::JSError(rt,
                                 "uniffi jsi player: read_string_from_buffer "
                                 "needs (buf, offset, length)");
            }
            auto obj = args[0].asObject(rt);
            jsi::ArrayBuffer buffer =
                obj.hasProperty(rt, "arrayBuffer")
                    ? obj.getPropertyAsObject(rt, "arrayBuffer")
                          .getArrayBuffer(rt)
                    : obj.getPropertyAsObject(rt, "buffer").getArrayBuffer(rt);
            auto offset = (size_t)args[1].asNumber();
            auto length = (size_t)args[2].asNumber();
            // Defensive bounds check: the TS codegen always passes in-range
            // (offset, length), but a corrupt buffer or out-of-range cast must
            // not read past the backing store.
            size_t bufSize = buffer.size(rt);
            if (offset > bufSize || length > bufSize - offset) {
              throw jsi::JSError(
                  rt,
                  "uniffi jsi player: read_string_from_buffer out of range");
            }
            return jsi::Value(rt, jsi::String::createFromUtf8(
                                      rt, buffer.data(rt) + offset, length));
          }));

  return jsi::Value(rt, mod);
}

// Process-wide count of player roots destroyed, exposed to JS as
// `uniffi.$teardownCount`. Diagnostic only: everything else the destructor
// below does is invisible from the runtime that replaces it, so this is the
// only way a test can prove the trigger fired at all.
std::atomic<uint64_t> g_rootTeardowns{0};

// The `uniffi` global.
//
// A HostObject rather than a plain Object because JSI destroys host objects
// with the runtime that owns them: ~UniffiPlayerRoot is the teardown hook a
// full reload otherwise does not give us, and no caller can forget to trigger
// it. Every module registered through this root is disarmed there, so a
// trampoline that outlives the runtime returns without touching it, rather than
// dereferencing a destroyed one. It zeroes whatever return bytes the callback
// has; an out_return callback has none, so Rust reads back
// FfiDefault::ffi_default() with call_status.code still 0.
//
// Every module object built from this root holds a strong reference back to it,
// so the set of armed modules and the root that disarms them live and die
// together, whatever JS does to the `uniffi` global.
//
// The destructor performs no VM operation — no jsi::Value, no property, no call
// into the runtime. JSI runs host-object dtors from inside the GC with no
// usable Runtime& guaranteed, so it may only touch the shim's own mutexes and a
// C flag, and must stay cheap.
class UniffiPlayerRoot : public jsi::HostObject,
                         public std::enable_shared_from_this<UniffiPlayerRoot> {
public:
  UniffiPlayerRoot(std::shared_ptr<facebook::react::CallInvoker> callInvoker,
                   std::thread::id jsThreadId)
      : callInvoker_(std::move(callInvoker)), jsThreadId_(jsThreadId) {}

  ~UniffiPlayerRoot() override {
    // Reachability makes this the runtime's own teardown (see abortModule), but
    // JSI reserves the right to finalize a host object on any thread, so the
    // thread is checked and not merely asserted: the shim ships with NDEBUG.
    const bool onJsThread = std::this_thread::get_id() == jsThreadId_;
    assert(onJsThread && "uniffi player root finalized off the JS thread");
    for (const auto &info : modules_) {
      // Per module, because one that fails must not strand the rest still
      // armed — and because a destructor that throws terminates.
      try {
        // abortModule releases workers whose posted tasks still hold pointers
        // into the frames those workers are about to pop, and only this thread
        // discards those tasks before they can run. Off it, leave the workers
        // parked: a stuck worker is a leak, a task writing a popped frame is
        // memory corruption. Disarm regardless — a flag and a hook, safe from
        // any thread, and the half that stops the next call reaching a dead
        // runtime. A worker released here loops straight back into another
        // call; what turns that call away is the aborted flag, which
        // cb_dispatch checks before posting, since the disarm below has not run
        // yet.
        if (onJsThread) {
          ubrn_cb::abortModule(*info);
        }
        ubrn_jsi_disarm(info->module);
      } catch (...) {
      }
    }
    g_rootTeardowns.fetch_add(1, std::memory_order_relaxed);
  }

  jsi::Value get(jsi::Runtime &rt, const jsi::PropNameID &name) override;

  // The invoker and JS thread of the runtime this root belongs to.
  const std::shared_ptr<facebook::react::CallInvoker> &callInvoker() const {
    return callInvoker_;
  }
  std::thread::id jsThreadId() const { return jsThreadId_; }

  // A registration is added before anything that can throw, so a module core
  // accepted is never left armed with no owner to disarm it.
  void addModule(std::shared_ptr<ubrn_cb::ModuleCallbackInfo> info) {
    modules_.push_back(std::move(info));
  }

private:
  std::shared_ptr<facebook::react::CallInvoker> callInvoker_;
  std::thread::id jsThreadId_;
  // An additional owner, not the only one: the module object's host-function
  // closures hold the same infos. Every owner is a JS object of this runtime,
  // so the infos die here — while the leaked trampoline userdata pointing at
  // them does not. That back-pointer outliving its target is exactly why
  // disarming each module above is not optional: the flag is what keeps the
  // dangling pointer from ever being read.
  std::vector<std::shared_ptr<ubrn_cb::ModuleCallbackInfo>> modules_;
};

// `register(definitions)` host function, bound to a library path.
jsi::Value makeRegister(jsi::Runtime &rt, std::string libPath,
                        std::shared_ptr<UniffiPlayerRoot> root) {
  return jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forUtf8(rt, "register"), 1,
      [libPath, root](jsi::Runtime &rt, const jsi::Value &,
                      const jsi::Value *args, size_t count) -> jsi::Value {
        if (count < 1 || !args[0].isObject()) {
          throw jsi::JSError(
              rt, "uniffi jsi player: register() needs a definitions object");
        }
        auto defs = args[0].getObject(rt);
        auto symbols = defs.getProperty(rt, "symbols").asObject(rt);
        auto allocS =
            symbols.getProperty(rt, "rustbuffer_alloc").asString(rt).utf8(rt);
        auto freeS =
            symbols.getProperty(rt, "rustbuffer_free").asString(rt).utf8(rt);
        auto fromS = symbols.getProperty(rt, "rustbuffer_from_bytes")
                         .asString(rt)
                         .utf8(rt);

        auto fnsObj = defs.getProperty(rt, "functions").asObject(rt);
        auto names = fnsObj.getPropertyNames(rt);
        size_t nFns = names.size(rt);

        auto fns = std::make_shared<std::vector<FnInfo>>();
        // The callback/struct layouts live as long as this runtime: the module
        // object's closures and the player root capture them, and both are JS
        // objects of the runtime registering here. Used on every vtable build.
        auto cbInfo = std::make_shared<ubrn_cb::ModuleCallbackInfo>();

        // C-side storage that must outlive ubrn_jsi_register. All raw pointers
        // in the spec structs point into these; they are freed when this scope
        // ends, AFTER ubrn_jsi_register has copied everything into core's owned
        // ModuleSpec.
        std::vector<std::string> nameStore; // function symbol names
        std::vector<std::vector<const char *>>
            tagNameStore; // per-fn arg tag-name arrays
        std::vector<std::vector<const char *>>
            argNameStore; // per-fn arg type-name arrays
        std::deque<std::string>
            strPool; // stable backing for all tag- and type-name strings
        std::vector<UbrnFunctionSpec> specs;
        nameStore.reserve(nFns);
        tagNameStore.reserve(nFns);
        argNameStore.reserve(nFns);
        specs.reserve(nFns);

        // Helper: intern a string in strPool and return a stable c_str(). A
        // deque never relocates its elements on growth, so the pointers stay
        // valid.
        auto intern = [&strPool](const std::string &s) -> const char * {
          strPool.push_back(s);
          return strPool.back().c_str();
        };

        // Helper: for Callback/Struct/Reference args, core needs the type name
        // (the referenced struct/callback). Build a parallel `const char*`
        // array (NULL for scalars) interned in strPool, push it into `store` so
        // it outlives ubrn_jsi_register, and return its data() pointer — or
        // nullptr when no arg is named (push an empty vector to keep `store`
        // 1:1).
        auto makeArgTypeNames =
            [&intern](const std::vector<ArgDesc> &descs,
                      std::vector<std::vector<const char *>> &store)
            -> const char *const * {
          bool anyNamed = false;
          for (const auto &d : descs) {
            if (!d.name.empty()) {
              anyNamed = true;
              break;
            }
          }
          if (!anyNamed) {
            store.push_back({});
            return nullptr;
          }
          std::vector<const char *> argNames(descs.size(), nullptr);
          for (size_t j = 0; j < descs.size(); j++) {
            if (!descs[j].name.empty())
              argNames[j] = intern(descs[j].name);
          }
          store.push_back(std::move(argNames));
          return store.back().data();
        };

        // Helper: the tag name is what core parses, so every arg carries one.
        // Build the full parallel `const char*` array interned in strPool, push
        // it into `store` so it outlives ubrn_jsi_register, and return its
        // data() pointer.
        auto makeArgTagNames =
            [&intern](const std::vector<ArgDesc> &descs,
                      std::vector<std::vector<const char *>> &store)
            -> const char *const * {
          std::vector<const char *> tagNames(descs.size(), nullptr);
          for (size_t j = 0; j < descs.size(); j++)
            tagNames[j] = intern(descs[j].tagName);
          store.push_back(std::move(tagNames));
          return store.back().data();
        };

        for (size_t i = 0; i < nFns; i++) {
          auto key = names.getValueAtIndex(rt, i).asString(rt).utf8(rt);
          auto f = fnsObj.getProperty(rt, key.c_str()).asObject(rt);
          auto argsArr = f.getProperty(rt, "args").asObject(rt).asArray(rt);
          size_t nArgs = argsArr.size(rt);

          FnInfo info;
          info.name = key;
          std::vector<ArgDesc> argDescs(nArgs);
          bool supported = true;
          for (size_t j = 0; j < nArgs; j++) {
            argDescs[j] = argDescFromDefObject(
                rt, argsArr.getValueAtIndex(rt, j).asObject(rt));
            if (argDescs[j].tag == UBRN_TY_UNSUPPORTED)
              supported = false;
          }
          info.args = argDescs;
          ArgDesc retDesc =
              argDescFromDefObject(rt, f.getProperty(rt, "ret").asObject(rt));
          info.retTag = retDesc.tag;
          info.retSize = retDesc.size;
          if (info.retTag == UBRN_TY_UNSUPPORTED)
            supported = false;
          info.hasRcs = f.getProperty(rt, "hasRustCallStatus").getBool();
          // Skip functions carrying a genuinely-unsupported arg/ret tag: a
          // Reference/MutReference to a non-Struct inner type, or any
          // unrecognized tag (see value_conv.h's argDescFromDefObject). Plain
          // Callback fn-ptr args ARE now marshalled. A skipped function is not
          // registered with core, not installed on the module object.
          if (!supported)
            continue;
          fns->push_back(info);

          nameStore.push_back(key);
          UbrnFunctionSpec s;
          s.name = nameStore.back().c_str();
          s.arg_tag_names = makeArgTagNames(argDescs, tagNameStore);
          s.n_args = nArgs;
          s.arg_type_names = makeArgTypeNames(argDescs, argNameStore);
          s.ret_tag_name = intern(retDesc.tagName);
          s.has_rust_call_status = info.hasRcs ? 1 : 0;
          specs.push_back(s);
        }

        // --- Parse callbacks: name -> { args, ret, hasRustCallStatus,
        // outReturn }.
        // A callback's CallbackShape needs the registered module (core owns the
        // arg layout), so the parsed pieces wait here until after register.
        struct PendingCallback {
          std::string name;
          std::vector<ArgDesc> args;
          ArgDesc ret;
          bool hasRcs;
          bool outReturn;
        };
        std::vector<PendingCallback> pendingCbs;
        std::vector<UbrnCallbackSpec> cbSpecs;
        std::vector<std::vector<const char *>> cbTagNameStore;
        std::vector<std::vector<const char *>> cbArgNameStore;
        if (defs.hasProperty(rt, "callbacks")) {
          auto cbObj = defs.getProperty(rt, "callbacks").asObject(rt);
          auto cbNames = cbObj.getPropertyNames(rt);
          size_t nCb = cbNames.size(rt);
          cbSpecs.reserve(nCb);
          cbTagNameStore.reserve(nCb);
          cbArgNameStore.reserve(nCb);
          pendingCbs.reserve(nCb);
          for (size_t i = 0; i < nCb; i++) {
            auto cbName = cbNames.getValueAtIndex(rt, i).asString(rt).utf8(rt);
            auto c = cbObj.getProperty(rt, cbName.c_str()).asObject(rt);
            auto argsArr = c.getProperty(rt, "args").asObject(rt).asArray(rt);
            size_t nArgs = argsArr.size(rt);

            std::vector<ArgDesc> argDescs(nArgs);
            for (size_t j = 0; j < nArgs; j++) {
              argDescs[j] = argDescFromDefObject(
                  rt, argsArr.getValueAtIndex(rt, j).asObject(rt));
            }
            ArgDesc retDesc =
                argDescFromDefObject(rt, c.getProperty(rt, "ret").asObject(rt));
            bool hasRcs = c.hasProperty(rt, "hasRustCallStatus") &&
                          c.getProperty(rt, "hasRustCallStatus").getBool();
            bool outReturn = c.hasProperty(rt, "outReturn") &&
                             c.getProperty(rt, "outReturn").getBool();

            pendingCbs.push_back(
                {cbName, argDescs, retDesc, hasRcs, outReturn});

            UbrnCallbackSpec s;
            s.name = intern(cbName);
            s.arg_tag_names = makeArgTagNames(argDescs, cbTagNameStore);
            s.n_args = nArgs;
            s.arg_type_names = makeArgTypeNames(argDescs, cbArgNameStore);
            s.ret_tag_name = intern(retDesc.tagName);
            s.has_rust_call_status = hasRcs ? 1 : 0;
            s.out_return = outReturn ? 1 : 0;
            // Carry the struct name for a Struct return (e.g. the out_return
            // UniffiForeignFuture struct) so core parses ret to Struct(name)
            // instead of erroring; NULL for scalar/void/RustBuffer returns.
            s.ret_type_name =
                retDesc.name.empty() ? nullptr : intern(retDesc.name);
            cbSpecs.push_back(s);
          }
        }

        // --- Parse structs: name -> [ { name, type: Callback(...) }, ... ].
        std::vector<UbrnStructSpec> structSpecs;
        std::vector<std::vector<UbrnStructField>> structFieldStore;
        if (defs.hasProperty(rt, "structs")) {
          auto structsObj = defs.getProperty(rt, "structs").asObject(rt);
          auto structNames = structsObj.getPropertyNames(rt);
          size_t nStruct = structNames.size(rt);
          structSpecs.reserve(nStruct);
          structFieldStore.reserve(nStruct);
          for (size_t i = 0; i < nStruct; i++) {
            auto structName =
                structNames.getValueAtIndex(rt, i).asString(rt).utf8(rt);
            auto fieldsArr = structsObj.getProperty(rt, structName.c_str())
                                 .asObject(rt)
                                 .asArray(rt);
            size_t nFields = fieldsArr.size(rt);

            ubrn_cb::StructLayout layout;
            ubrn_cb::StructDesc structDesc;
            std::vector<UbrnStructField> fields;
            fields.reserve(nFields);
            for (size_t j = 0; j < nFields; j++) {
              auto fieldObj = fieldsArr.getValueAtIndex(rt, j).asObject(rt);
              auto fieldName =
                  fieldObj.getProperty(rt, "name").asString(rt).utf8(rt);
              ArgDesc typeDesc = argDescFromDefObject(
                  rt, fieldObj.getProperty(rt, "type").asObject(rt));
              UbrnStructField sf;
              sf.field_name = intern(fieldName);
              // "Callback" for a vtable's method fields.
              sf.type_tag_name = intern(typeDesc.tagName);
              sf.type_name =
                  typeDesc.name.empty() ? nullptr : intern(typeDesc.name);
              fields.push_back(sf);
              // The C++-side vtable builder only cares about Callback fields
              // (the JS method -> trampoline mapping). Record (method,
              // callback).
              if (typeDesc.tag == UBRN_TY_CALLBACK) {
                layout.fields.emplace_back(fieldName, typeDesc.name);
              }
              // Record EVERY field (name + type) for struct-by-value
              // marshalling (ForeignFutureResult<T>, the out_return
              // UniffiForeignFuture struct).
              structDesc.fields.push_back({fieldName, typeDesc});
            }
            cbInfo->structs.emplace(structName, std::move(layout));
            cbInfo->structDescs.emplace(structName, std::move(structDesc));

            structFieldStore.push_back(std::move(fields));
            UbrnStructSpec s;
            s.name = intern(structName);
            s.fields = structFieldStore.back().data();
            s.n_fields = structFieldStore.back().size();
            structSpecs.push_back(s);
          }
        }

        UbrnModuleSpec spec;
        spec.rustbuffer_alloc = allocS.c_str();
        spec.rustbuffer_free = freeS.c_str();
        spec.rustbuffer_from_bytes = fromS.c_str();
        spec.functions = specs.data();
        spec.n_functions = specs.size();
        spec.callbacks = cbSpecs.empty() ? nullptr : cbSpecs.data();
        spec.n_callbacks = cbSpecs.size();
        spec.structs = structSpecs.empty() ? nullptr : structSpecs.data();
        spec.n_structs = structSpecs.size();

        char err[512] = {0};
        UbrnJsiModule *handle =
            ubrn_jsi_register(libPath.c_str(), &spec, err, sizeof(err));
        if (!handle) {
          throw jsi::JSError(
              rt, std::string("uniffi jsi player: register failed: ") + err);
        }
        // `handle` is never freed: it is captured by the module object's
        // host-function closures and by leaked trampoline userdata, either of
        // which can outlive the runtime that registered it. The root disarms it
        // when that runtime goes away, which is what makes those survivors
        // inert rather than dangerous.
        cbInfo->module = handle;
        cbInfo->bindRuntime(root->callInvoker(), root->jsThreadId());
        root->addModule(cbInfo);

        // Now that core knows every callback, cache each shape (with core's
        // slot offsets) for vtable building / callback invocation.
        for (const auto &p : pendingCbs) {
          cbInfo->callbacks.emplace(
              p.name, ubrn_cb::buildShape(rt, handle, p.name, p.args, p.ret,
                                          p.hasRcs, p.outReturn));
        }

        return buildModuleObject(rt, handle, fns, cbInfo, root);
      });
}

jsi::Value UniffiPlayerRoot::get(jsi::Runtime &rt,
                                 const jsi::PropNameID &name) {
  auto prop = name.utf8(rt);
  if (prop == "open") {
    // The closure keeps the root alive for as long as JS holds the returned
    // function. Not a retain cycle: the root does not own the function, and
    // both die with the runtime.
    auto self = shared_from_this();
    return jsi::Function::createFromHostFunction(
        rt, jsi::PropNameID::forUtf8(rt, "open"), 1,
        [self](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
               size_t count) -> jsi::Value {
          if (count < 1 || !args[0].isString()) {
            throw jsi::JSError(rt,
                               "uniffi jsi player: open() needs a path string");
          }
          std::string path = args[0].getString(rt).utf8(rt);
          jsi::Object handle(rt);
          handle.setProperty(rt, "register", makeRegister(rt, path, self));
          return jsi::Value(rt, handle);
        });
  }
  if (prop == "$teardownCount") {
    return jsi::Value((double)g_rootTeardowns.load(std::memory_order_relaxed));
  }
  return jsi::Value::undefined();
}

void installUniffiHostObject(
    jsi::Runtime &rt,
    std::shared_ptr<facebook::react::CallInvoker> callInvoker) {
  auto root = std::make_shared<UniffiPlayerRoot>(std::move(callInvoker),
                                                 std::this_thread::get_id());
  rt.global().setProperty(
      rt, "uniffi", jsi::Object::createFromHostObject(rt, std::move(root)));
}

} // namespace

// The symbol the Hermes test-runner (and the production TurboModule) calls.
extern "C" void
registerNatives(jsi::Runtime &rt,
                std::shared_ptr<facebook::react::CallInvoker> callInvoker) {
  installUniffiHostObject(rt, std::move(callInvoker));
}
