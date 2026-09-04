/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#include "callbacks.h"

#include <atomic>

namespace ubrn_cb {

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

CallbackShape buildShape(jsi::Runtime &rt, UbrnJsiModule *module,
                         const std::string &cbName,
                         const std::vector<ArgDesc> &args, const ArgDesc &ret,
                         bool hasRcs, bool outReturn) {
  CallbackShape shape;
  shape.args = args;
  shape.hasRcs = hasRcs;
  shape.outReturn = outReturn;
  shape.retTag = ret.tag;
  shape.retName = ret.name;
  shape.retTagSize = ret.size;
  // ret_size: 0 for void or out_return, else the scalar/RustBuffer slot size.
  shape.retSize = (outReturn || ret.tag == UBRN_TY_VOID) ? 0 : shape.retTagSize;

  // Core lays out the buffer its trampoline packs, in CIF order:
  // [declared args, out_return ptr?, RustCallStatus ptr?].
  const size_t nSlots =
      args.size() + (outReturn ? 1u : 0u) + (hasRcs ? 1u : 0u);
  std::vector<size_t> offsets(nSlots);
  std::vector<size_t> sizes(nSlots);
  int n = ubrn_jsi_callback_arg_layout(module, cbName.c_str(), &shape.totalSize,
                                       offsets.data(), sizes.data(), nSlots);
  if (n < 0) {
    // Core has no flat arg buffer for this signature — a struct passed by
    // value, as in ForeignFutureComplete*. It refuses a trampoline for the
    // same reason, so such a callback is only ever invoked as an incoming fn
    // pointer, which reads `args`, never the slots below. Leave them empty.
    return shape;
  }
  if ((size_t)n != nSlots) {
    // Core and the shim disagree on how many slots this signature has, so any
    // offset read out of the truncated arrays would be the wrong one.
    throw jsi::JSError(rt, "uniffi jsi player: callback '" + cbName + "' has " +
                               std::to_string(n) + " slots in core, " +
                               std::to_string(nSlots) + " here");
  }

  shape.argSlots.reserve(args.size());
  for (size_t i = 0; i < args.size(); i++) {
    shape.argSlots.push_back(SlotLayout{offsets[i], sizes[i]});
  }
  size_t next = args.size();
  if (outReturn) {
    shape.outReturnSlot = SlotLayout{offsets[next], sizes[next]};
    next++;
  }
  if (hasRcs) {
    shape.rcsSlot = SlotLayout{offsets[next], sizes[next]};
  }
  shape.slotsValid = true;
  return shape;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

namespace {

// Read a pointer-sized value from a slot.
inline void *readPointer(const uint8_t *p) {
  void *v;
  memcpy(&v, p, sizeof(v));
  return v;
}

// Wrap an incoming Rust fn pointer (e.g. a future completer) as a JS function.
// Defined after marshalJsStructToBytes (it marshals struct-by-value args).
// Ports marshal.rs::create_fn_pointer_wrapper.
jsi::Value makeFnPointerWrapper(jsi::Runtime &rt, UbrnJsiModule *module,
                                const ModuleCallbackInfo &info,
                                const std::string &cbName, const void *fnPtr);

// Build the JS arg list from the flat byte buffer per the callback's arg descs.
// Ports read_arg_bytes_to_js (scalars + RustBuffer + Callback-as-fn-ptr). A
// Callback-typed incoming arg is a raw Rust fn ptr (a future completer) wrapped
// as a callable JS function via makeFnPointerWrapper.
jsi::Value readArgToJs(jsi::Runtime &rt, UbrnJsiModule *module,
                       const ModuleCallbackInfo *info, const ArgDesc &desc,
                       const uint8_t *argBytes) {
  if (desc.tag == UBRN_TY_CALLBACK) {
    const void *fnPtr = readPointer(argBytes);
    if (info == nullptr) {
      throw jsi::JSError(
          rt, "uniffi jsi player: callback arg requires module info");
    }
    return makeFnPointerWrapper(rt, module, *info, desc.name, fnPtr);
  }
  if (desc.tag == UBRN_TY_RUSTBUFFER) {
    UbrnRustBuffer rb;
    memcpy(&rb, argBytes, sizeof(rb));
    // Copy the bytes into a fresh JS Uint8Array, then free the Rust buffer —
    // mirrors NAPI read_arg_bytes_to_js for RustBuffer (JS owns its own copy).
    size_t len = (size_t)rb.len;
    auto ctor = rt.global().getPropertyAsFunction(rt, "Uint8Array");
    auto arr = ctor.callAsConstructor(rt, (double)len).asObject(rt);
    if (len > 0 && rb.data != nullptr) {
      auto ab = arr.getPropertyAsObject(rt, "buffer").getArrayBuffer(rt);
      memcpy(ab.data(rt), rb.data, len);
    }
    if (rb.data != nullptr) {
      ubrn_jsi_rustbuffer_free(module, rb);
    }
    return jsi::Value(rt, arr);
  }
  // Scalars / handles. bytesToScalar handles the BigInt path for 64-bit.
  return bytesToScalar(rt, desc.tag, argBytes);
}

// Write a JS value (already lowered by the codegen) into a return-byte buffer
// of `size` bytes, per `tag`. For RustBuffer, the JS value is a Uint8Array
// which we copy into a Rust buffer and store as its repr(C) UbrnRustBuffer
// (size is target-dependent). Ports write_js_return_to_bytes.
void writeJsToBytes(jsi::Runtime &rt, UbrnJsiModule *module, uint8_t tag,
                    const jsi::Value &v, uint8_t *dst, size_t size) {
  if (tag == UBRN_TY_RUSTBUFFER) {
    UbrnRustBuffer rb = rustBufferForArg(rt, module, v);
    size_t copy = sizeof(rb) < size ? sizeof(rb) : size;
    memcpy(dst, &rb, copy);
    return;
  }
  scalarToBytes(rt, tag, v, dst);
}

// Pull the repr(C) UbrnRustBuffer out of a JS Uint8Array (errBuf) into a Rust
// buffer, for the RustCallStatus error path.
UbrnRustBuffer errBufToRustBuffer(jsi::Runtime &rt, UbrnJsiModule *module,
                                  const jsi::Value &v) {
  return rustBufferForArg(rt, module, v);
}

// Forward decl (mutual recursion: struct fields can be nested structs).
void marshalFieldToBytes(jsi::Runtime &rt, UbrnJsiModule *module,
                         const ModuleCallbackInfo &info, const ArgDesc &type,
                         const jsi::Value &v, uint8_t *slot, size_t slotSize);

} // namespace

// ---------------------------------------------------------------------------
// Struct-by-value marshalling (ports marshal.rs::marshal_js_struct_to_bytes).
// ---------------------------------------------------------------------------

std::vector<uint8_t> marshalJsStructToBytes(jsi::Runtime &rt,
                                            UbrnJsiModule *module,
                                            const ModuleCallbackInfo &info,
                                            const std::string &structName,
                                            const jsi::Object &jsObj) {
  auto descIt = info.structDescs.find(structName);
  if (descIt == info.structDescs.end()) {
    throw jsi::JSError(rt, "uniffi jsi player: unknown struct: " + structName);
  }
  const StructDesc &desc = descIt->second;

  // Field byte offsets/sizes + total size from core (libffi).
  size_t nFields = desc.fields.size();
  size_t totalSize = 0;
  std::vector<size_t> offsets(nFields ? nFields : 1, 0);
  std::vector<size_t> sizes(nFields ? nFields : 1, 0);
  int realN = ubrn_jsi_struct_field_offsets(
      module, structName.c_str(), &totalSize, offsets.data(), sizes.data(),
      nFields ? nFields : 1);
  if (realN < 0 || (size_t)realN != nFields) {
    throw jsi::JSError(
        rt, "uniffi jsi player: struct_field_offsets failed for " + structName);
  }

  std::vector<uint8_t> buf(totalSize, 0);
  for (size_t i = 0; i < nFields; i++) {
    const StructFieldDesc &field = desc.fields[i];
    auto fieldVal = jsObj.getProperty(rt, field.name.c_str());
    marshalFieldToBytes(rt, module, info, field.type, fieldVal,
                        buf.data() + offsets[i], sizes[i]);
  }
  return buf;
}

namespace {

// Marshal a single struct field's JS value into the given byte slot.
// Ports marshal.rs::marshal_field_to_bytes.
void marshalFieldToBytes(jsi::Runtime &rt, UbrnJsiModule *module,
                         const ModuleCallbackInfo &info, const ArgDesc &type,
                         const jsi::Value &v, uint8_t *slot, size_t slotSize) {
  switch (type.tag) {
  case UBRN_TY_RUSTBUFFER: {
    UbrnRustBuffer rb = rustBufferForArg(rt, module, v);
    size_t copy = sizeof(rb) < slotSize ? sizeof(rb) : slotSize;
    memcpy(slot, &rb, copy);
    return;
  }
  case UBRN_TY_STRUCT: {
    // Nested registered struct. Recurse, then copy into the field slot.
    if (!v.isObject())
      return; // zero-filled slot is a valid empty struct
    auto nested =
        marshalJsStructToBytes(rt, module, info, type.name, v.asObject(rt));
    size_t copy = nested.size() < slotSize ? nested.size() : slotSize;
    memcpy(slot, nested.data(), copy);
    return;
  }
  case UBRN_TY_RUSTCALLSTATUS: {
    // Inline RustCallStatus: {i8 code, u64 capacity, u64 len, *u8 data} with
    // natural alignment (code@0, then padding, capacity@8, len@16, data@24).
    // JS shape is { code, errorBuf? }. Not a registered struct, so handle
    // inline (mirrors marshal.rs's RustCallStatus arm). slot is already
    // zero-filled.
    if (!v.isObject())
      return; // zero == success status
    auto obj = v.asObject(rt);
    int8_t code = 0;
    if (obj.hasProperty(rt, "code")) {
      auto c = obj.getProperty(rt, "code");
      if (c.isNumber())
        code = (int8_t)(int)c.asNumber();
    }
    if (slotSize >= 1)
      slot[0] = (uint8_t)code;
    if (code != 0 && obj.hasProperty(rt, "errorBuf")) {
      auto eb = obj.getProperty(rt, "errorBuf");
      if (eb.isObject()) {
        UbrnRustBuffer rb = errBufToRustBuffer(rt, module, eb);
        // capacity@8, len@16, data@24 within the field slot.
        if (slotSize >= 32)
          memcpy(slot + 8, &rb, sizeof(rb));
      }
    }
    return;
  }
  case UBRN_TY_CALLBACK: {
    const void *fnPtr = trampolineForJsFn(rt, module, info, type.name, v,
                                          "struct field '" + type.name + "'");
    memcpy(slot, &fnPtr, sizeof(fnPtr));
    return;
  }
  default:
    // Scalars / Handle. scalarToBytes writes the tag's slot width.
    scalarToBytes(rt, type.tag, v, slot);
    return;
  }
}

// Marshal one JS arg to its C byte representation, for invoking a fn pointer.
// Ports marshal.rs::marshal_arg_to_bytes. A Struct arg is marshalled by value.
std::vector<uint8_t> marshalArgToBytes(jsi::Runtime &rt, UbrnJsiModule *module,
                                       const ModuleCallbackInfo &info,
                                       const ArgDesc &desc,
                                       const jsi::Value &v) {
  if (desc.tag == UBRN_TY_STRUCT) {
    return marshalJsStructToBytes(rt, module, info, desc.name, v.asObject(rt));
  }
  if (desc.tag == UBRN_TY_RUSTBUFFER) {
    UbrnRustBuffer rb = rustBufferForArg(rt, module, v);
    std::vector<uint8_t> out(sizeof(rb));
    memcpy(out.data(), &rb, sizeof(rb));
    return out;
  }
  // Scalars / Handle / pointer-sized.
  size_t sz = desc.size;
  std::vector<uint8_t> out(sz ? sz : 1, 0);
  scalarToBytes(rt, desc.tag, v, out.data());
  out.resize(sz);
  return out;
}

// The identity a JS function is known by in core's trampoline reuse map.
//
// NativeState rather than a property: it lives in an internal slot, so it is
// invisible to Object.keys, spreads and JSON.stringify — the reuse rule must
// stay unobservable from JS — and it is never inherited, so a function whose
// prototype is another callback cannot borrow its identity and be handed the
// wrong trampoline. Same mechanism as RustBufferOwner in value_conv.h.
class TrampolineId : public jsi::NativeState {
public:
  explicit TrampolineId(uint64_t id) : id(id) {}
  const uint64_t id;
};

// Source of those identities. Process-global because the stash is: one JS
// function holds one NativeState slot and is marshalled by every module that
// takes it, so an id must be unique across all of their maps. Starts at 1: 0 is
// trampolineForJsFn's "no identity" sentinel. Atomic because a process can run
// more than one JS runtime, each on its own thread.
std::atomic<uint64_t> g_nextTrampolineId{1};

// Wrap an incoming Rust fn pointer as a callable JS function: when JS calls it,
// each arg is marshalled to C bytes and the fn ptr is invoked through core's
// ubrn_jsi_call_callback_ptr. Ports marshal.rs::create_fn_pointer_wrapper.
jsi::Value makeFnPointerWrapper(jsi::Runtime &rt, UbrnJsiModule *module,
                                const ModuleCallbackInfo &info,
                                const std::string &cbName, const void *fnPtr) {
  auto cbIt = info.callbacks.find(cbName);
  if (cbIt == info.callbacks.end()) {
    throw jsi::JSError(rt,
                       "uniffi jsi player: unknown callback '" + cbName + "'");
  }
  // Borrow the declared arg descs (the completer's signature) by pointer: they
  // live in `info.callbacks`, which the module object's closures and the
  // player root both own via shared_ptr. The root is the last of those to go,
  // and its destructor disarms before the registry is freed, so a call that
  // gets this far reads descs that are still alive.
  const std::vector<ArgDesc> *argTypes = &cbIt->second.args;
  const ModuleCallbackInfo *infoPtr = &info;
  return jsi::Function::createFromHostFunction(
      rt, jsi::PropNameID::forUtf8(rt, "fn_pointer_wrapper"),
      (unsigned)argTypes->size(),
      [module, cbName, argTypes, fnPtr,
       infoPtr](jsi::Runtime &rt, const jsi::Value &, const jsi::Value *args,
                size_t count) -> jsi::Value {
        size_t nArgs = argTypes->size();
        if (count < nArgs) {
          throw jsi::JSError(rt, "uniffi jsi player: completer '" + cbName +
                                     "' expects " + std::to_string(nArgs) +
                                     " args, got " + std::to_string(count));
        }
        std::vector<uint8_t> blob;
        std::vector<size_t> sizes(nArgs);
        for (size_t i = 0; i < nArgs; i++) {
          std::vector<uint8_t> chunk =
              marshalArgToBytes(rt, module, *infoPtr, (*argTypes)[i], args[i]);
          sizes[i] = chunk.size();
          blob.insert(blob.end(), chunk.begin(), chunk.end());
        }
        int rc = ubrn_jsi_call_callback_ptr(module, cbName.c_str(), fnPtr,
                                            blob.data(), sizes.data(), nArgs);
        if (rc != 0) {
          throw jsi::JSError(rt,
                             "uniffi jsi player: call_callback_ptr failed (" +
                                 std::to_string(rc) + ") for '" + cbName + "'");
        }
        return jsi::Value::undefined();
      });
}

} // namespace

// ---------------------------------------------------------------------------
// trampolineForJsFn — the single Callback-marshal site (declared in
// callbacks.h).
// ---------------------------------------------------------------------------

const void *trampolineForJsFn(jsi::Runtime &rt, UbrnJsiModule *module,
                              const ModuleCallbackInfo &info,
                              const std::string &cbName, const jsi::Value &v,
                              const std::string &errLabel) {
  auto cbIt = info.callbacks.find(cbName);
  if (cbIt == info.callbacks.end()) {
    throw jsi::JSError(rt,
                       "uniffi jsi player: unknown callback '" + cbName + "'");
  }
  if (!v.isObject() || !v.asObject(rt).isFunction(rt)) {
    throw jsi::JSError(rt,
                       "uniffi jsi player: " + errLabel + " is not a function");
  }
  auto jsFnObj = v.asObject(rt).getFunction(rt);

  // The identity core's reuse map keys on, alongside the callback name. Read
  // the stash before minting: a fresh identity per marshal would build a
  // trampoline per call, which is the leak reuse exists to prevent. 0 means
  // "no identity" — minted ones start at 1 — and marshals without memoising.
  //
  // Two function classes never get an identity and so leak one CbUserData plus
  // one libffi closure per marshal — unbounded growth for that input, not
  // merely "no reuse": one already carrying another owner's NativeState, whose
  // slot must not be overwritten, and one that refuses the write (frozen or
  // proxied). Accepted because neither is common as a callback in practice.
  uint64_t identity = 0;
  if (jsFnObj.hasNativeState(rt)) {
    auto stashed =
        std::dynamic_pointer_cast<TrampolineId>(jsFnObj.getNativeState(rt));
    if (stashed) {
      identity = stashed->id;
    }
  } else {
    uint64_t minted =
        g_nextTrampolineId.fetch_add(1, std::memory_order_relaxed);
    try {
      jsFnObj.setNativeState(rt, std::make_shared<TrampolineId>(minted));
      identity = minted;
    } catch (const jsi::JSIException &) {
    }
  }

  // Nothing below runs on a hit; see ModuleCallbackInfo for why reuse is safe.
  if (identity != 0) {
    if (const void *hit =
            ubrn_jsi_trampoline_for(module, cbName.c_str(), identity)) {
      return hit;
    }
  }

  auto jsFn = std::make_shared<jsi::Function>(std::move(jsFnObj));
  auto *ud = new CbUserData{&rt,
                            jsFn,
                            cbIt->second,
                            module,
                            &info,
                            info.callInvoker,
                            info.jsThreadId,
                            info.abortState};
  const void *fnPtr =
      ubrn_jsi_make_trampoline(module, cbName.c_str(), cb_on_js_thread,
                               cb_dispatch, cb_is_js_thread, ud);
  if (fnPtr == nullptr) {
    throw jsi::JSError(rt, "uniffi jsi player: make_trampoline failed for '" +
                               cbName + "'");
  }
  info.trampolinesBuilt++;
  if (identity != 0) {
    ubrn_jsi_remember_trampoline(module, cbName.c_str(), identity, fnPtr);
  }
  return fnPtr;
}

// ---------------------------------------------------------------------------
// cb_on_js_thread — runs on the JS thread (see core trampoline protocol).
// ---------------------------------------------------------------------------

// The body. Everything below can throw: the JS callback itself, an
// unmarshallable argument, an unknown struct, an already-consumed buffer.
static void cb_on_js_thread_impl(const uint8_t *args, uint8_t *ret,
                                 const void *udPtr) {
  const auto *ud = static_cast<const CbUserData *>(udPtr);
  jsi::Runtime &rt = *ud->rt;
  const CallbackShape &shape = ud->shape;

  // A shape with no slots would make every read below index past the end of
  // argSlots. It cannot reach here — core computes the same layout before it
  // builds a trampoline, so a callback it gives no layout for gets no
  // trampoline either — but that invariant lives in core, so enforce it here
  // rather than trusting it. Returning, not throwing: core reaches this through
  // a plain `extern "C"` fn pointer on the same-thread path, where an exception
  // would unwind into Rust. Every synchronous vtable method is out_return, so
  // ret_size is 0 here: Rust reads back FfiDefault::ffi_default() with
  // call_status.code still 0, a successful empty return, not a reported
  // failure.
  if (!shape.slotsValid) {
    return;
  }

  size_t declared = shape.args.size();

  // Read declared args -> JS values.
  std::vector<jsi::Value> jsArgs;
  jsArgs.reserve(declared + 1);
  for (size_t i = 0; i < declared; i++) {
    const auto &slot = shape.argSlots[i];
    jsArgs.push_back(readArgToJs(rt, ud->module, ud->info, shape.args[i],
                                 args + slot.offset));
  }

  // Resolve the out_return and RustCallStatus pointers from their slots.
  void *outReturnPtr = nullptr;
  if (shape.outReturn) {
    outReturnPtr = readPointer(args + shape.outReturnSlot.offset);
  }
  RustCallStatus *statusPtr = nullptr;
  if (shape.hasRcs) {
    statusPtr = reinterpret_cast<RustCallStatus *>(
        readPointer(args + shape.rcsSlot.offset));
  }

  // Non-out_return + hasRcs: append a {code} status object (pass-by-reference).
  // (Not used by the `callbacks` fixture's vtable methods, which are all
  // out_return; kept for parity with NAPI.)
  bool appendedStatus = false;
  if (shape.hasRcs && !shape.outReturn) {
    jsi::Object js_status(rt);
    int code = statusPtr ? (int)statusPtr->code : 0;
    js_status.setProperty(rt, "code", jsi::Value((double)code));
    jsArgs.push_back(jsi::Value(rt, js_status));
    appendedStatus = true;
  }

  // Call the JS method. jsArgs.data() is already a jsi::Value*; bind to the
  // (const Value*, size_t) overload explicitly (a non-const Value* would match
  // the variadic template and fail to compile).
  const jsi::Value *callArgsPtr = jsArgs.data();
  jsi::Value result = ud->jsFn->call(rt, callArgsPtr, jsArgs.size());

  if (shape.outReturn) {
    if (!result.isObject())
      return;
    auto obj = result.getObject(rt);

    // Direct struct return (no RustCallStatus): JS returns the struct object
    // itself (e.g. UniffiForeignFuture { handle, free }). Marshal it by value
    // and write through the out_return pointer. Ports
    // write_js_value_to_pointer.
    if (!shape.hasRcs) {
      if (outReturnPtr != nullptr && shape.retTag == UBRN_TY_STRUCT &&
          ud->info != nullptr) {
        auto bytes = marshalJsStructToBytes(rt, ud->module, *ud->info,
                                            shape.retName, obj);
        memcpy(outReturnPtr, bytes.data(), bytes.size());
      } else if (outReturnPtr != nullptr && shape.retTag != UBRN_TY_VOID &&
                 shape.retTag != UBRN_TY_STRUCT) {
        writeJsToBytes(rt, ud->module, shape.retTag, result,
                       static_cast<uint8_t *>(outReturnPtr), shape.retTagSize);
      }
      return;
    }

    // UniffiResult protocol: JS returns { code, pointee?, errorBuf? }.
    int code = 0;
    if (obj.hasProperty(rt, "code")) {
      auto c = obj.getProperty(rt, "code");
      if (c.isNumber())
        code = (int)c.asNumber();
    }
    if (statusPtr) {
      statusPtr->code = (int8_t)code;
      if (code != 0 && obj.hasProperty(rt, "errorBuf")) {
        auto eb = obj.getProperty(rt, "errorBuf");
        if (eb.isObject()) {
          statusPtr->error_buf = errBufToRustBuffer(rt, ud->module, eb);
        }
      }
    }
    // Write the pointee on success (code == 0) when there is a real return.
    if (code == 0 && outReturnPtr != nullptr && shape.retTag != UBRN_TY_VOID &&
        obj.hasProperty(rt, "pointee")) {
      auto pointee = obj.getProperty(rt, "pointee");
      if (shape.retTag == UBRN_TY_STRUCT && ud->info != nullptr) {
        auto bytes = marshalJsStructToBytes(
            rt, ud->module, *ud->info, shape.retName, pointee.asObject(rt));
        memcpy(outReturnPtr, bytes.data(), bytes.size());
      } else {
        writeJsToBytes(rt, ud->module, shape.retTag, pointee,
                       static_cast<uint8_t *>(outReturnPtr), shape.retTagSize);
      }
    }
    return;
  }

  // Non-out_return path.
  if (shape.hasRcs && statusPtr && appendedStatus) {
    // Read back the mutated code from the status object we appended.
    auto &statusVal = jsArgs.back();
    if (statusVal.isObject()) {
      auto so = statusVal.getObject(rt);
      if (so.hasProperty(rt, "code")) {
        auto c = so.getProperty(rt, "code");
        if (c.isNumber())
          statusPtr->code = (int8_t)(int)c.asNumber();
      }
      if (statusPtr->code != 0 && so.hasProperty(rt, "errorBuf")) {
        auto eb = so.getProperty(rt, "errorBuf");
        if (eb.isObject()) {
          statusPtr->error_buf = errBufToRustBuffer(rt, ud->module, eb);
        }
      }
    }
  }
  if (ret != nullptr && shape.retSize > 0) {
    writeJsToBytes(rt, ud->module, shape.retTag, result, ret, shape.retSize);
  }
}

// Core invokes this through a plain `extern "C"` fn pointer, which rustc marks
// nounwind — on the same-thread path it is called straight from
// `trampoline_body`. A C++ exception crossing that frame is undefined
// behaviour, so nothing may escape here. Every synchronous vtable method is
// out_return, so ret_size is 0 here: Rust reads back FfiDefault::ffi_default()
// with call_status.code still 0, a successful empty return, not a reported
// failure.
extern "C" void cb_on_js_thread(const uint8_t *args, uint8_t *ret,
                                const void *udPtr) {
  try {
    cb_on_js_thread_impl(args, ret, udPtr);
  } catch (...) {
  }
}

// ---------------------------------------------------------------------------
// cb_dispatch — runs on a worker thread; rendezvous onto the JS thread.
// ---------------------------------------------------------------------------

namespace {

// One cross-thread callback's transfer buffer: the arg bytes copied off the
// worker's stack, the return bytes the JS thread writes, and the flag the
// rendezvous waits on. Heap-owned and shared with the posted task, so these two
// buffers outlive the worker's frame even when an abort releases it early.
//
// That is the whole of what heap-owning buys. The out_return and
// RustCallStatus pointers travel *inside* the arg bytes and address the Rust
// caller's own locals, so a task running after its worker has returned still
// writes a popped frame. Only abortModule's precondition rules that out.
struct DispatchSlot {
  std::vector<uint8_t> args;
  std::vector<uint8_t> ret;
  bool done = false;
};

} // namespace

void abortModule(const ModuleCallbackInfo &info) {
  const auto &state = info.abortState;
  if (state == nullptr) {
    return;
  }
  {
    std::lock_guard<std::mutex> lk(state->mtx);
    state->aborted = true;
  }
  state->cv.notify_all();
}

static void cb_dispatch_impl(UbrnOnJsThreadFn on_js, const uint8_t *args,
                             uint8_t *ret, const void *udPtr) {
  const auto *ud = static_cast<const CbUserData *>(udPtr);
  const CallbackShape &shape = ud->shape;

  // Same guard as cb_on_js_thread, applied before posting: a slotless shape
  // would copy a zero-length buffer and cb_on_js_thread would then index past
  // the end of argSlots. Returning here leaves ret_size at 0 for the common
  // out_return case, so Rust reads back FfiDefault::ffi_default() with
  // call_status.code still 0 — a successful empty return, not a reported
  // failure.
  if (!shape.slotsValid) {
    return;
  }

  // Every trampoline carries the invoker and release valve of the runtime it
  // was built for. Checked, not asserted: release builds define NDEBUG, and the
  // dereferences below would be null on a worker thread. Returning here leaves
  // ret_size at 0 for the out_return case, so Rust reads back
  // FfiDefault::ffi_default() with call_status.code still 0 — a successful
  // empty return, not a reported failure.
  const auto &state = ud->abortState;
  if (ud->callInvoker == nullptr || state == nullptr) {
    return;
  }

  // Already torn down: post nothing. This userdata keeps the invoker alive past
  // its runtime, so the call below would otherwise succeed and queue a task
  // nothing will ever drain.
  //
  // A filter, not a barrier: an abort landing between this check and the post
  // still queues that orphan. What keeps the worker itself safe is the wait
  // predicate, which re-reads `aborted` under the same mutex and so returns on
  // a signal already given rather than parking on one that has passed. Holding
  // the mutex across the post would close the window, at the price of calling
  // into the host's scheduler underneath one of our own locks.
  {
    std::lock_guard<std::mutex> lk(state->mtx);
    if (state->aborted) {
      return;
    }
  }

  // Copy the FULL arg buffer so the worker thread can release its stack. core's
  // trampoline lays it out as [declared_args, out_return_ptr?, RCS_ptr?];
  // shape.totalSize covers all of those (matching core's ArgLayout::total_size
  // and the NAPI oracle's arg_layout.total_size). Using only argSlots.back()
  // would truncate the out_return and RCS pointer slots, so cb_on_js_thread
  // would read those pointers past the end of the copy.
  auto slot = std::make_shared<DispatchSlot>();
  slot->args.resize(shape.totalSize);
  if (shape.totalSize > 0 && args != nullptr) {
    memcpy(slot->args.data(), args, shape.totalSize);
  }
  slot->ret.resize(shape.retSize);

  const void *capturedUd = udPtr;
  auto captured = state;

  // A callback with nothing to hand back -- the rust_future continuation and
  // vtable free -- is posted and forgotten. uniffi invokes the continuation
  // while holding the future's scheduler mutex, so waiting here for the JS
  // thread deadlocks whenever that thread is itself inside rust_future_poll
  // and the poll wakes another future: it blocks on the mutex this worker
  // holds while this worker blocks on it.
  if (!shape.hasRcs && !shape.outReturn && shape.retSize == 0) {
    ud->callInvoker->invokeAsync([on_js, slot, capturedUd](jsi::Runtime &) {
      try {
        on_js(slot->args.data(), nullptr, capturedUd);
      } catch (...) {
      }
    });
    return;
  }

  // Rendezvous: post onto the JS thread via invokeAsync, block until it runs or
  // the module is aborted. NEVER invokeSync (it is a no-op in the test
  // harness). uniffi foreign-thread callbacks fire from Rust-owned threads, so
  // the JS thread is free to drain.
  ud->callInvoker->invokeAsync(
      [on_js, slot, captured, capturedUd](jsi::Runtime &) {
        // on_js is cb_on_js_thread, already wrapped in its own catch(...)
        // barrier — this try/catch is belt-and-braces. The worker is parked
        // until done is set, so the signal has to happen on every path out of
        // this task or that worker waits for an abort instead. Returning here
        // leaves ret_size at 0 for the out_return case, so Rust reads back
        // FfiDefault::ffi_default() with call_status.code still 0 — a
        // successful empty return, not a reported failure.
        try {
          on_js(slot->args.data(),
                slot->ret.empty() ? nullptr : slot->ret.data(), capturedUd);
        } catch (...) {
        }
        {
          std::lock_guard<std::mutex> lk(captured->mtx);
          slot->done = true;
        }
        // notify_all, not notify_one: this condvar is shared by every
        // cross-thread callback of the module, so waking one arbitrary waiter
        // can wake a worker whose own task has not run and leave this one
        // parked.
        captured->cv.notify_all();
      });

  {
    std::unique_lock<std::mutex> lk(state->mtx);
    state->cv.wait(lk, [&] { return slot->done || state->aborted; });
    // The completing task fills slot->ret and then publishes done under this
    // mutex, so a wake on done sees the fill. A wake on aborted races nothing:
    // abort runs on the thread that drains this invoker, immediately before
    // the runtime dies, so no queued task ever runs. slot->ret is then still
    // the zero fill from resize, which is what core's unloading path produces.
    if (shape.retSize > 0 && ret != nullptr) {
      memcpy(ret, slot->ret.data(), shape.retSize);
    }
  }
}

// Core invokes this through a plain `extern "C"` fn pointer, which rustc marks
// nounwind — `trampoline_body` calls it directly on the foreign-thread path. A
// C++ exception crossing that frame is undefined behaviour, and everything
// above allocates, so nothing may escape here.
extern "C" void cb_dispatch(UbrnOnJsThreadFn on_js, const uint8_t *args,
                            uint8_t *ret, const void *udPtr) {
  try {
    cb_dispatch_impl(on_js, args, ret, udPtr);
  } catch (...) {
  }
}

// ---------------------------------------------------------------------------
// cb_is_js_thread
// ---------------------------------------------------------------------------

// The answer is per-runtime, not per-process: two runtimes in one process each
// have their own JS thread, and a trampoline belongs to exactly one of them.
//
// `udPtr` is the pointer handed to ubrn_jsi_make_trampoline, which is always a
// live CbUserData, so it is dereferenced unconditionally here as it is in
// cb_on_js_thread and cb_dispatch. Answering `false` for a null instead would
// only route the call into those two, which dereference it anyway.
extern "C" bool cb_is_js_thread(const void *udPtr) {
  const auto *ud = static_cast<const CbUserData *>(udPtr);
  return std::this_thread::get_id() == ud->jsThreadId;
}

// ---------------------------------------------------------------------------
// buildVTableStruct — ports vtable.rs::build_vtable_struct.
// ---------------------------------------------------------------------------

const void *buildVTableStruct(jsi::Runtime &rt, UbrnJsiModule *module,
                              const ModuleCallbackInfo &info,
                              const std::string &structName,
                              const jsi::Object &jsObj) {
  auto structIt = info.structs.find(structName);
  if (structIt == info.structs.end()) {
    throw jsi::JSError(rt, "uniffi jsi player: unknown vtable struct: " +
                               structName);
  }
  const StructLayout &layout = structIt->second;

  // The callback name pointers refer to `layout.fields[*].second`, owned by
  // `info.structs`, which lives as long as the registering runtime — so they
  // stay valid for the duration of (and well past) the ubrn_jsi_build_vtable
  // call below.
  std::vector<const char *> callbackNamePtrs;
  std::vector<const void *> fnPtrs;
  callbackNamePtrs.reserve(layout.fields.size());
  fnPtrs.reserve(layout.fields.size());

  for (const auto &field : layout.fields) {
    const std::string &methodName = field.first;
    const std::string &callbackName = field.second;

    auto cbIt = info.callbacks.find(callbackName);
    if (cbIt == info.callbacks.end()) {
      throw jsi::JSError(rt, "uniffi jsi player: vtable field '" + methodName +
                                 "' references unknown callback '" +
                                 callbackName + "'");
    }

    // The JS function comes from the method-named property, but the callback
    // shape is looked up by callbackName — so pass the value separately and
    // keep the descriptive "vtable field '<method>'" error label. The userdata
    // is heap-allocated + LEAKED (process lifetime): the Rust library may
    // invoke the vtable at any future time, from any thread.
    auto methodVal = jsObj.getProperty(rt, methodName.c_str());
    const void *fnPtr =
        trampolineForJsFn(rt, module, info, callbackName, methodVal,
                          "vtable field '" + methodName + "'");
    callbackNamePtrs.push_back(callbackName.c_str());
    fnPtrs.push_back(fnPtr);
  }

  const void *vtable =
      ubrn_jsi_build_vtable(module, structName.c_str(), callbackNamePtrs.data(),
                            fnPtrs.data(), fnPtrs.size());
  if (vtable == nullptr) {
    throw jsi::JSError(rt,
                       "uniffi jsi player: build_vtable failed for struct '" +
                           structName + "'");
  }
  return vtable;
}

} // namespace ubrn_cb
