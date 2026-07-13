/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#include <ReactCommon/CallInvoker.h>
#include <jsi/jsi.h>

#include <cstdint>
#include <cstring>
#include <deque>
#include <memory>
#include <string>
#include <thread>
#include <vector>

#include "callbacks.h" // Rust -> JS callback machinery (vtable building, the three fns)
#include "ubrn_jsi.h" // from runtimes/jsi/include
#include "value_conv.h" // tagSize / scalarToBytes / bytesToScalar / arrayBytes / ArgDesc

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
  bool hasRcs;
};

// A jsi buffer that owns a returned Rust RustBuffer and frees it on GC.
//
// Lifetime DECISION: the RustBuffer's lifetime is tied to the Uint8Array's GC
// via this MutableBuffer, which makes the codegen-emitted `rustbuffer_free`
// call a no-op (see buildModuleObject). This is simpler than NAPI's eager-free
// + capacity-symbol scheme and avoids a double-free on the aliased view: the
// view aliases the Rust allocation directly, so freeing it eagerly while JS
// still holds the view would be use-after-free. If a later fixture
// (gc-callbacks-crasher / coverall leak checks, Task 3.5) proves a leak, switch
// to NAPI's model (eager free + capacity symbol).
class RustOwnedBuffer : public jsi::MutableBuffer {
public:
  RustOwnedBuffer(UbrnJsiModule *m, UbrnRustBuffer rb) : m_(m), rb_(rb) {}
  ~RustOwnedBuffer() override { ubrn_jsi_rustbuffer_free(m_, rb_); }
  size_t size() const override { return (size_t)rb_.len; }
  uint8_t *data() override { return rb_.data; }

private:
  UbrnJsiModule *m_;
  UbrnRustBuffer rb_;
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
// Note: UBRN_TY_UNSUPPORTED, ArgDesc and argDescFromDefObject now live in
// value_conv.h (shared with the callback machinery).

// Build the native module object from a parsed DEFINITIONS and a registered
// handle.
jsi::Value
buildModuleObject(jsi::Runtime &rt, UbrnJsiModule *handle,
                  std::shared_ptr<std::vector<FnInfo>> fns,
                  std::shared_ptr<ubrn_cb::ModuleCallbackInfo> cbInfo) {
  jsi::Object mod(rt);
  for (const auto &fn : *fns) {
    FnInfo info = fn; // copy into the closure
    auto hostFn = jsi::Function::createFromHostFunction(
        rt, jsi::PropNameID::forUtf8(rt, info.name),
        (unsigned)info.args.size() + (info.hasRcs ? 1 : 0),
        [handle, info, cbInfo](jsi::Runtime &rt, const jsi::Value &,
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
          // into a Rust-owned buffer (via rustbuffer_from_bytes) whose 24-byte
          // repr(C) layout is then stored as the arg payload.
          std::vector<std::vector<uint8_t>> backing(nDeclared);
          std::vector<const void *> argPtrs(nDeclared);
          std::vector<size_t> argSizes(nDeclared);
          for (size_t i = 0; i < nDeclared; i++) {
            uint8_t tag = info.args[i].tag;
            size_t sz = tagSize(tag);
            backing[i].resize(sz);
            if (tag == UBRN_TY_RUSTBUFFER) {
              auto [ptr, len] = arrayBytes(rt, args[i]);
              UbrnRustBuffer rb =
                  ubrn_jsi_rustbuffer_from_bytes(handle, ptr, len);
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
              // TODO: rust_future_poll_* is called once per poll, so this leaks
              // a fresh CbUserData + trampoline PER POLL (unbounded for
              // long-running futures). Acceptable for host-parity scope and
              // matches the NAPI oracle's known behavior; a future
              // free-after-invoke mechanism (distribution plan) would address
              // it.
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

          size_t retSize = tagSize(info.retTag);
          std::vector<uint8_t> out(retSize ? retSize : 1, 0);

          int rc = ubrn_jsi_call(handle, info.name.c_str(), argPtrs.data(),
                                 argSizes.data(), nDeclared, statusPtr,
                                 out.data(), retSize);
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

          // A RustBuffer return is the 24-byte repr(C) UbrnRustBuffer written
          // into `out`. Alias it as a Uint8Array whose backing memory is freed
          // when the JS view is GC'd (see RustOwnedBuffer).
          if (info.retTag == UBRN_TY_RUSTBUFFER) {
            UbrnRustBuffer rb;
            memcpy(&rb, out.data(), sizeof(rb));
            return rustBufferToUint8Array(rt, handle, rb);
          }

          return bytesToScalar(rt, info.retTag, out.data());
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
            std::string s = args[0].asString(rt).utf8(rt);
            size_t len = s.size();
            // Allocate a JS-owned Uint8Array(len) and copy the UTF-8 bytes into
            // its backing ArrayBuffer. (Mirrors the error-buf copy path above;
            // avoids a native-owned MutableBuffer and any Bridging.h dependency.)
            auto ctor = rt.global().getPropertyAsFunction(rt, "Uint8Array");
            auto arr = ctor.callAsConstructor(rt, (double)len).asObject(rt);
            if (len) {
              auto ab = arr.getPropertyAsObject(rt, "buffer").getArrayBuffer(rt);
              memcpy(ab.data(rt), s.data(), len);
            }
            return jsi::Value(rt, arr);
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

// `register(definitions)` host function, bound to a library path.
jsi::Value makeRegister(jsi::Runtime &rt, std::string libPath) {
  return jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forUtf8(rt, "register"), 1,
      [libPath](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
                size_t count) -> jsi::Value {
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
        // The callback/struct layouts are kept alive for the process lifetime
        // (captured by the module object's closures, used on every vtable
        // build).
        auto cbInfo = std::make_shared<ubrn_cb::ModuleCallbackInfo>();

        // C-side storage that must outlive ubrn_jsi_register. All raw pointers
        // in the spec structs point into these; they are freed when this scope
        // ends, AFTER ubrn_jsi_register has copied everything into core's owned
        // ModuleSpec.
        std::vector<std::string> nameStore; // function symbol names
        std::vector<std::vector<uint8_t>> tagStore;
        std::vector<std::vector<const char *>>
            argNameStore; // per-fn arg type-name arrays
        std::deque<std::string>
            strPool; // stable backing for all type-name strings
        std::vector<UbrnFunctionSpec> specs;
        nameStore.reserve(nFns);
        tagStore.reserve(nFns);
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

        for (size_t i = 0; i < nFns; i++) {
          auto key = names.getValueAtIndex(rt, i).asString(rt).utf8(rt);
          auto f = fnsObj.getProperty(rt, key.c_str()).asObject(rt);
          auto argsArr = f.getProperty(rt, "args").asObject(rt).asArray(rt);
          size_t nArgs = argsArr.size(rt);

          FnInfo info;
          info.name = key;
          std::vector<ArgDesc> argDescs(nArgs);
          std::vector<uint8_t> tags(nArgs);
          bool supported = true;
          for (size_t j = 0; j < nArgs; j++) {
            argDescs[j] = argDescFromDefObject(
                rt, argsArr.getValueAtIndex(rt, j).asObject(rt));
            tags[j] = argDescs[j].tag;
            if (tags[j] == UBRN_TY_UNSUPPORTED)
              supported = false;
          }
          info.args = argDescs;
          ArgDesc retDesc =
              argDescFromDefObject(rt, f.getProperty(rt, "ret").asObject(rt));
          info.retTag = retDesc.tag;
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
          tagStore.push_back(tags);
          UbrnFunctionSpec s;
          s.name = nameStore.back().c_str();
          s.arg_tags = tagStore.back().data();
          s.n_args = nArgs;
          s.arg_type_names = makeArgTypeNames(argDescs, argNameStore);
          s.ret_tag = info.retTag;
          s.has_rust_call_status = info.hasRcs ? 1 : 0;
          specs.push_back(s);
        }

        // --- Parse callbacks: name -> { args, ret, hasRustCallStatus,
        // outReturn }.
        std::vector<UbrnCallbackSpec> cbSpecs;
        std::vector<std::vector<uint8_t>> cbTagStore;
        std::vector<std::vector<const char *>> cbArgNameStore;
        if (defs.hasProperty(rt, "callbacks")) {
          auto cbObj = defs.getProperty(rt, "callbacks").asObject(rt);
          auto cbNames = cbObj.getPropertyNames(rt);
          size_t nCb = cbNames.size(rt);
          cbSpecs.reserve(nCb);
          cbTagStore.reserve(nCb);
          cbArgNameStore.reserve(nCb);
          for (size_t i = 0; i < nCb; i++) {
            auto cbName = cbNames.getValueAtIndex(rt, i).asString(rt).utf8(rt);
            auto c = cbObj.getProperty(rt, cbName.c_str()).asObject(rt);
            auto argsArr = c.getProperty(rt, "args").asObject(rt).asArray(rt);
            size_t nArgs = argsArr.size(rt);

            std::vector<ArgDesc> argDescs(nArgs);
            std::vector<uint8_t> tags(nArgs);
            for (size_t j = 0; j < nArgs; j++) {
              argDescs[j] = argDescFromDefObject(
                  rt, argsArr.getValueAtIndex(rt, j).asObject(rt));
              tags[j] = argDescs[j].tag;
            }
            ArgDesc retDesc =
                argDescFromDefObject(rt, c.getProperty(rt, "ret").asObject(rt));
            bool hasRcs = c.hasProperty(rt, "hasRustCallStatus") &&
                          c.getProperty(rt, "hasRustCallStatus").getBool();
            bool outReturn = c.hasProperty(rt, "outReturn") &&
                             c.getProperty(rt, "outReturn").getBool();

            // Cache the shape for vtable building / callback invocation.
            cbInfo->callbacks.emplace(
                cbName, ubrn_cb::buildShape(argDescs, retDesc.tag, retDesc.name,
                                            hasRcs, outReturn));

            cbTagStore.push_back(tags);
            UbrnCallbackSpec s;
            s.name = intern(cbName);
            s.arg_tags = cbTagStore.back().data();
            s.n_args = nArgs;
            s.arg_type_names = makeArgTypeNames(argDescs, cbArgNameStore);
            s.ret_tag = retDesc.tag;
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
              sf.type_tag = typeDesc.tag; // UBRN_TY_CALLBACK for vtable methods
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
        // `handle` is intentionally not freed here: it is captured by the
        // module object's host-function closures and lives for the process
        // lifetime of this test-runner shim. A future module-teardown lifecycle
        // (distribution packaging phase) will call ubrn_jsi_free at unload
        // time.
        return buildModuleObject(rt, handle, fns, cbInfo);
      });
}

void installUniffiHostObject(jsi::Runtime &rt) {
  jsi::Object uniffi(rt);
  // open(path) -> { register(defs) -> nativeModule }
  auto openFn = jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forUtf8(rt, "open"), 1,
      [](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
         size_t count) -> jsi::Value {
        if (count < 1 || !args[0].isString()) {
          throw jsi::JSError(rt,
                             "uniffi jsi player: open() needs a path string");
        }
        std::string path = args[0].getString(rt).utf8(rt);
        jsi::Object handle(rt);
        handle.setProperty(rt, "register", makeRegister(rt, path));
        return jsi::Value(rt, handle);
      });
  uniffi.setProperty(rt, "open", openFn);
  rt.global().setProperty(rt, "uniffi", uniffi);
}

} // namespace

// The symbol the Hermes test-runner (and the production TurboModule) calls.
extern "C" void
registerNatives(jsi::Runtime &rt,
                std::shared_ptr<facebook::react::CallInvoker> callInvoker) {
  // Capture the CallInvoker + JS thread id for cross-thread Rust -> JS
  // callbacks (see callbacks.cpp). Same-thread callbacks bypass the invoker
  // entirely.
  ubrn_cb::g_callInvoker = std::move(callInvoker);
  ubrn_cb::g_jsThreadId = std::this_thread::get_id();
  installUniffiHostObject(rt);
}
