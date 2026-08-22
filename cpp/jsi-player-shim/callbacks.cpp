/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#include "callbacks.h"

#include <atomic>
#include <cassert>

namespace ubrn_cb {

std::shared_ptr<facebook::react::CallInvoker> g_callInvoker;
std::thread::id g_jsThreadId;

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

CallbackShape buildShape(const std::vector<ArgDesc> &args, uint8_t retTag,
                         const std::string &retName, bool hasRcs,
                         bool outReturn) {
  CallbackShape shape;
  shape.args = args;
  shape.hasRcs = hasRcs;
  shape.outReturn = outReturn;
  shape.retTag = retTag;
  shape.retName = retName;

  size_t offset = 0;
  auto place = [&](uint8_t tag) -> SlotLayout {
    size_t size = tagSize(tag);
    size_t align = tagAlign(tag);
    offset = (offset + align - 1) & ~(align - 1);
    SlotLayout slot{offset, size};
    offset += size;
    return slot;
  };

  shape.argSlots.reserve(args.size());
  for (const auto &a : args) {
    shape.argSlots.push_back(place(a.tag));
  }
  // out_return appears as an extra VoidPointer (pointer-sized) arg slot, before
  // the RustCallStatus slot — matching core::ArgLayout when out_return is set.
  if (outReturn) {
    shape.outReturnSlot = place(UBRN_TY_HANDLE); // pointer-sized
  }
  if (hasRcs) {
    shape.rcsSlot = place(UBRN_TY_HANDLE); // *mut RustCallStatus, pointer-sized
  }
  // After placing every slot, `offset` is the end of the full buffer —
  // exactly core::ArgLayout::compute's total_size for
  // [declared_args, out_return_ptr?, RCS_ptr?].
  shape.totalSize = offset;

  // ret_size: 0 for void or out_return, else the scalar/RustBuffer slot size.
  if (outReturn || retTag == UBRN_TY_VOID) {
    shape.retSize = 0;
  } else {
    shape.retSize = tagSize(retTag);
  }
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
// which we copy into a Rust buffer and store as its 24-byte repr(C). Ports
// write_js_return_to_bytes.
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

// Pull the 24-byte UbrnRustBuffer out of a JS Uint8Array (errBuf) into a Rust
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
    // Scalars / Handle. scalarToBytes writes tagSize(tag) bytes.
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
  size_t sz = tagSize(desc.tag);
  std::vector<uint8_t> out(sz ? sz : 1, 0);
  scalarToBytes(rt, desc.tag, v, out.data());
  out.resize(sz);
  return out;
}

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
  // live in `info.callbacks` (process-lifetime ModuleCallbackInfo, captured
  // into the module closures via shared_ptr), so they outlive every completer
  // call.
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

  // Reuse this function's trampoline if it already has one. Nothing below runs
  // on a hit; see ModuleCallbackInfo::trampolines for why reuse is safe and why
  // a scan beats building a closure.
  for (const auto &entry : info.trampolines) {
    if (entry.cbName == cbName &&
        jsi::Object::strictEquals(rt, *entry.fn, jsFnObj)) {
      return entry.fnPtr;
    }
  }

  auto jsFn = std::make_shared<jsi::Function>(std::move(jsFnObj));
  auto *ud = new CbUserData{&rt, jsFn, cbIt->second, module, &info};
  const void *fnPtr =
      ubrn_jsi_make_trampoline(module, cbName.c_str(), cb_on_js_thread,
                               cb_dispatch, cb_is_js_thread, ud);
  if (fnPtr == nullptr) {
    throw jsi::JSError(rt, "uniffi jsi player: make_trampoline failed for '" +
                               cbName + "'");
  }
  info.trampolines.push_back({cbName, jsFn, fnPtr});
  return fnPtr;
}

// ---------------------------------------------------------------------------
// cb_on_js_thread — runs on the JS thread (see core trampoline protocol).
// ---------------------------------------------------------------------------

extern "C" void cb_on_js_thread(const uint8_t *args, uint8_t *ret,
                                const void *udPtr) {
  const auto *ud = static_cast<const CbUserData *>(udPtr);
  jsi::Runtime &rt = *ud->rt;
  const CallbackShape &shape = ud->shape;

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
        size_t size = tagSize(shape.retTag);
        writeJsToBytes(rt, ud->module, shape.retTag, result,
                       static_cast<uint8_t *>(outReturnPtr), size);
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
        size_t size = tagSize(shape.retTag);
        writeJsToBytes(rt, ud->module, shape.retTag, pointee,
                       static_cast<uint8_t *>(outReturnPtr), size);
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

// ---------------------------------------------------------------------------
// cb_dispatch — runs on a worker thread; rendezvous onto the JS thread.
// ---------------------------------------------------------------------------

extern "C" void cb_dispatch(UbrnOnJsThreadFn on_js, const uint8_t *args,
                            uint8_t *ret, const void *udPtr) {
  const auto *ud = static_cast<const CbUserData *>(udPtr);
  const CallbackShape &shape = ud->shape;

  // Copy the FULL arg buffer so the calling (worker) thread can release its
  // stack. core's trampoline lays it out as [declared_args, out_return_ptr?,
  // RCS_ptr?]; shape.totalSize covers all of those (matching core's
  // ArgLayout::total_size and the NAPI oracle's arg_layout.total_size). Using
  // only argSlots.back() would truncate the out_return and RCS pointer slots,
  // so cb_on_js_thread would read those pointers past the end of the copy.
  size_t argsLen = shape.totalSize;
  std::vector<uint8_t> argsCopy(argsLen);
  if (argsLen > 0 && args != nullptr) {
    memcpy(argsCopy.data(), args, argsLen);
  }
  size_t retLen = shape.retSize;
  std::vector<uint8_t> retBuf(retLen);

  // Rendezvous: post onto the JS thread via invokeAsync, block until it runs.
  // NEVER invokeSync (it is a no-op in the test harness). uniffi foreign-thread
  // callbacks fire from Rust-owned threads, so the JS thread is free to drain.
  auto mtx = std::make_shared<std::mutex>();
  auto cv = std::make_shared<std::condition_variable>();
  auto done = std::make_shared<bool>(false);

  // Capture raw pointers/spans by value into the task.
  const uint8_t *argsPtr = argsCopy.data();
  uint8_t *retPtr = retLen > 0 ? retBuf.data() : nullptr;
  const void *capturedUd = udPtr;

  // Invariant: registerNatives ran first and captured the host CallInvoker.
  // A cross-thread callback before that would be a wiring bug.
  assert(g_callInvoker != nullptr &&
         "cb_dispatch: g_callInvoker not set (registerNatives must run first)");

  g_callInvoker->invokeAsync([=](jsi::Runtime &) {
    on_js(argsPtr, retPtr, capturedUd);
    {
      std::lock_guard<std::mutex> lk(*mtx);
      *done = true;
    }
    cv->notify_one();
  });

  {
    std::unique_lock<std::mutex> lk(*mtx);
    cv->wait(lk, [&] { return *done; });
  }

  if (retLen > 0 && ret != nullptr) {
    memcpy(ret, retBuf.data(), retLen);
  }
}

// ---------------------------------------------------------------------------
// cb_is_js_thread
// ---------------------------------------------------------------------------

extern "C" bool cb_is_js_thread(const void *) {
  return std::this_thread::get_id() == g_jsThreadId;
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
  // `info.structs` (process-lifetime ModuleCallbackInfo), so they stay valid
  // for the duration of (and well past) the ubrn_jsi_build_vtable call below.
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
