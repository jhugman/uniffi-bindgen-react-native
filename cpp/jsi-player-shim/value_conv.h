/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// value_conv.h — jsi::Value <-> native bytes for the player.
//
// Houses the scalar marshalling helpers (moved out of shim.cpp to keep it
// readable) plus the RustBuffer <-> bytes plumbing. The RustBuffer->Uint8Array
// view conversion (which needs the module handle, to free on GC) lives in
// shim.cpp because it depends on the host-function lifetime model.
#pragma once
#include <jsi/jsi.h>

#include <cstdint>
#include <cstring>
#include <string>
#include <utility>
#include <vector>

#include "ubrn_jsi.h"

namespace jsi = facebook::jsi;

// 24-byte UbrnRustBuffer matches core::RustBufferC exactly.
static_assert(sizeof(UbrnRustBuffer) == 24, "RustBuffer ABI mismatch");

// Sentinel returned for FfiType tags the player cannot marshal at all.
// Functions whose signature contains an unsupported tag are skipped at
// registration time (e.g. a Reference/MutReference to a non-Struct inner type,
// or any unrecognized tag — see argDescFromDefObject below).
constexpr uint8_t UBRN_TY_UNSUPPORTED = 0xFF;

// One argument's type, carrying the name for Callback/Struct/Reference tags.
// For scalars/RustBuffer `name` is empty. `tag` is a UbrnFfiType (or
// UBRN_TY_UNSUPPORTED).
struct ArgDesc {
  uint8_t tag = UBRN_TY_VOID;
  std::string name; // callback/struct name for named tags, else empty
};

// Classify a DEFINITIONS `{ tag, name?, inner? }` object into an ArgDesc.
//   Callback  -> (UBRN_TY_CALLBACK, name)
//   Struct    -> (UBRN_TY_STRUCT, name)
//   Reference(Struct(name)) -> (UBRN_TY_REFERENCE, name)  (vtable pointer arg)
// Scalars/RustBuffer carry an empty name. Anything else -> UBRN_TY_UNSUPPORTED.
inline ArgDesc argDescFromDefObject(jsi::Runtime &rt, const jsi::Object &o) {
  auto tag = o.getProperty(rt, "tag").asString(rt).utf8(rt);
  ArgDesc d;
  if (tag == "Void") {
    d.tag = UBRN_TY_VOID;
    return d;
  }
  if (tag == "UInt8") {
    d.tag = UBRN_TY_U8;
    return d;
  }
  if (tag == "Int8") {
    d.tag = UBRN_TY_I8;
    return d;
  }
  if (tag == "UInt16") {
    d.tag = UBRN_TY_U16;
    return d;
  }
  if (tag == "Int16") {
    d.tag = UBRN_TY_I16;
    return d;
  }
  if (tag == "UInt32") {
    d.tag = UBRN_TY_U32;
    return d;
  }
  if (tag == "Int32") {
    d.tag = UBRN_TY_I32;
    return d;
  }
  if (tag == "UInt64") {
    d.tag = UBRN_TY_U64;
    return d;
  }
  if (tag == "Int64") {
    d.tag = UBRN_TY_I64;
    return d;
  }
  if (tag == "Float32") {
    d.tag = UBRN_TY_F32;
    return d;
  }
  if (tag == "Float64") {
    d.tag = UBRN_TY_F64;
    return d;
  }
  if (tag == "Handle") {
    d.tag = UBRN_TY_HANDLE;
    return d;
  }
  if (tag == "RustBuffer") {
    d.tag = UBRN_TY_RUSTBUFFER;
    return d;
  }
  // A VoidPointer is pointer-sized; we treat it like a Handle for byte layout.
  // (Appears as a callback's out_return arg type / void return; never needs a
  // name.)
  if (tag == "VoidPointer") {
    d.tag = UBRN_TY_HANDLE;
    return d;
  }
  // RustCallStatus only appears as an inline struct field (e.g. inside
  // ForeignFutureResult<T>); its byte layout is computed by core via
  // struct_field_offsets, so it needs no tagSize entry of its own.
  if (tag == "RustCallStatus") {
    d.tag = UBRN_TY_RUSTCALLSTATUS;
    return d;
  }
  if (tag == "Callback") {
    d.tag = UBRN_TY_CALLBACK;
    d.name = o.getProperty(rt, "name").asString(rt).utf8(rt);
    return d;
  }
  if (tag == "Struct") {
    d.tag = UBRN_TY_STRUCT;
    d.name = o.getProperty(rt, "name").asString(rt).utf8(rt);
    return d;
  }
  if (tag == "Reference" || tag == "MutReference") {
    auto inner = o.getProperty(rt, "inner").asObject(rt);
    auto innerTag = inner.getProperty(rt, "tag").asString(rt).utf8(rt);
    if (innerTag == "Struct") {
      d.tag = UBRN_TY_REFERENCE;
      d.name = inner.getProperty(rt, "name").asString(rt).utf8(rt);
      return d;
    }
    d.tag = UBRN_TY_UNSUPPORTED;
    return d;
  }
  d.tag = UBRN_TY_UNSUPPORTED;
  return d;
}

// Byte width of a type tag. RustBuffer marshals as its 24-byte repr(C) layout.
inline size_t tagSize(uint8_t tag) {
  switch (tag) {
  case UBRN_TY_VOID:
    return 0;
  case UBRN_TY_U8:
  case UBRN_TY_I8:
    return 1;
  case UBRN_TY_U16:
  case UBRN_TY_I16:
    return 2;
  case UBRN_TY_U32:
  case UBRN_TY_I32:
  case UBRN_TY_F32:
    return 4;
  case UBRN_TY_U64:
  case UBRN_TY_I64:
  case UBRN_TY_F64:
  case UBRN_TY_HANDLE:
    return 8;
  // Callback/Struct(pointer)/Reference all marshal as a pointer-sized slot in a
  // callback's flat arg buffer (mirrors core::slot_size_align for these).
  case UBRN_TY_CALLBACK:
  case UBRN_TY_STRUCT:
  case UBRN_TY_REFERENCE:
    return sizeof(void *);
  case UBRN_TY_RUSTBUFFER:
    return sizeof(UbrnRustBuffer); // 24
  default:
    return 0;
  }
}

// Natural alignment of a tag's slot. Mirrors core::slot_size_align: every
// primitive/pointer slot has align == size; RustBuffer aligns to 8 (its first
// field is u64). Used to reproduce core's ArgLayout offsets on the C++ side.
inline size_t tagAlign(uint8_t tag) {
  switch (tag) {
  case UBRN_TY_VOID:
    return 1;
  case UBRN_TY_RUSTBUFFER:
    return 8; // RustBufferC{u64,u64,*}
  default:
    return tagSize(tag) ? tagSize(tag) : 1;
  }
}

// Read the raw 64-bit pattern from a jsi value for a U64/I64/Handle slot. The
// generated TS lowers these via FfiConverterUInt64/Int64, which produce JS
// BigInt values (Arc pointers, handles, 64-bit ints). Hermes throws
// "Value is a bigint, expected a number" from asNumber() on a BigInt, so we
// must take the BigInt path. A plain number is also accepted (small 64-bit
// literals can arrive un-promoted); getUint64() truncates to the low 64 bits,
// which is the correct bit pattern for both signed and unsigned slots.
inline uint64_t bits64FromValue(jsi::Runtime &rt, const jsi::Value &v) {
  if (v.isBigInt())
    return v.getBigInt(rt).getUint64(rt);
  if (v.isBool())
    return v.getBool() ? 1 : 0;
  return (uint64_t)(int64_t)v.asNumber();
}

// Write a jsi number/bool/bigint into `dst` (size tagSize(tag)) as native
// bytes.
inline void scalarToBytes(jsi::Runtime &rt, uint8_t tag, const jsi::Value &v,
                          uint8_t *dst) {
  switch (tag) {
  case UBRN_TY_U64:
  case UBRN_TY_I64:
  case UBRN_TY_HANDLE: {
    uint64_t x = bits64FromValue(rt, v);
    memcpy(dst, &x, 8);
    return;
  }
  default:
    break;
  }
  double d = v.isBool() ? (v.getBool() ? 1.0 : 0.0) : v.asNumber();
  switch (tag) {
  case UBRN_TY_U8: {
    uint8_t x = (uint8_t)d;
    memcpy(dst, &x, 1);
    break;
  }
  case UBRN_TY_I8: {
    int8_t x = (int8_t)d;
    memcpy(dst, &x, 1);
    break;
  }
  case UBRN_TY_U16: {
    uint16_t x = (uint16_t)d;
    memcpy(dst, &x, 2);
    break;
  }
  case UBRN_TY_I16: {
    int16_t x = (int16_t)d;
    memcpy(dst, &x, 2);
    break;
  }
  case UBRN_TY_U32: {
    uint32_t x = (uint32_t)d;
    memcpy(dst, &x, 4);
    break;
  }
  case UBRN_TY_I32: {
    int32_t x = (int32_t)d;
    memcpy(dst, &x, 4);
    break;
  }
  case UBRN_TY_F32: {
    float x = (float)d;
    memcpy(dst, &x, 4);
    break;
  }
  case UBRN_TY_F64: {
    double x = d;
    memcpy(dst, &x, 8);
    break;
  }
  default:
    break;
  }
}

// Read native bytes back into a jsi number.
inline jsi::Value bytesToScalar(jsi::Runtime &rt, uint8_t tag,
                                const uint8_t *src) {
  switch (tag) {
  case UBRN_TY_VOID:
    return jsi::Value::undefined();
  case UBRN_TY_U8: {
    uint8_t x;
    memcpy(&x, src, 1);
    return jsi::Value((double)x);
  }
  case UBRN_TY_I8: {
    int8_t x;
    memcpy(&x, src, 1);
    return jsi::Value((double)x);
  }
  case UBRN_TY_U16: {
    uint16_t x;
    memcpy(&x, src, 2);
    return jsi::Value((double)x);
  }
  case UBRN_TY_I16: {
    int16_t x;
    memcpy(&x, src, 2);
    return jsi::Value((double)x);
  }
  case UBRN_TY_U32: {
    uint32_t x;
    memcpy(&x, src, 4);
    return jsi::Value((double)x);
  }
  case UBRN_TY_I32: {
    int32_t x;
    memcpy(&x, src, 4);
    return jsi::Value((double)x);
  }
  case UBRN_TY_F32: {
    float x;
    memcpy(&x, src, 4);
    return jsi::Value((double)x);
  }
  // 64-bit ints/handles cross into JS as BigInt: the generated TS reads them
  // via FfiConverterUInt64/Int64 (getBigUint64/getBigInt64). Returning a double
  // would lose precision past 2^53 and mistype the value (BigInt vs number).
  case UBRN_TY_U64:
  case UBRN_TY_HANDLE: {
    uint64_t x;
    memcpy(&x, src, 8);
    return jsi::Value(jsi::BigInt::fromUint64(rt, x));
  }
  case UBRN_TY_I64: {
    int64_t x;
    memcpy(&x, src, 8);
    return jsi::Value(jsi::BigInt::fromInt64(rt, x));
  }
  case UBRN_TY_F64: {
    double x;
    memcpy(&x, src, 8);
    return jsi::Value(x);
  }
  default:
    return jsi::Value::undefined();
  }
}

// Extract (ptr, len) from a jsi Uint8Array/ArrayBuffer argument.
//
// For a Uint8Array view we must honour byteOffset/byteLength: the codegen-
// emitted lower() path can hand back a view that is a sub-range of a larger
// ArrayBuffer, and the FFI call must see only the message bytes.
inline std::pair<const uint8_t *, size_t> arrayBytes(jsi::Runtime &rt,
                                                     const jsi::Value &v) {
  auto obj = v.asObject(rt);
  jsi::ArrayBuffer ab =
      obj.isArrayBuffer(rt)
          ? obj.getArrayBuffer(rt)
          : obj.getPropertyAsObject(rt, "buffer").getArrayBuffer(rt);
  size_t byteOffset = 0, byteLength = ab.size(rt);
  if (!obj.isArrayBuffer(rt)) {
    byteOffset = (size_t)obj.getProperty(rt, "byteOffset").asNumber();
    byteLength = (size_t)obj.getProperty(rt, "byteLength").asNumber();
  }
  return {ab.data(rt) + byteOffset, byteLength};
}

// ---------------------------------------------------------------------------
// RustBuffer ownership across the JS boundary
// ---------------------------------------------------------------------------

// A jsi buffer that owns a library allocation and frees it when the JS view is
// collected — unless the allocation has been handed to a callee first.
//
// Two kinds of Uint8Array reach an FFI argument, and they must be treated
// differently (this mirrors NAPI's js_uint8array_to_rust_buffer):
//
//   * Library-owned views, produced by `rustbuffer_alloc`. Codegen allocates
//     one, fills it in place and passes it straight through. These are
//     ADOPTED: the existing allocation goes to the callee, which frees it.
//     Copying instead would allocate and memcpy a second buffer per call and
//     leave the original alive until the view happened to be collected.
//   * Ordinary JS arrays, which are not ours to give away. These are COPIED
//     into a fresh library allocation.
//
// `release()` is what makes adoption safe: it disarms the destructor, so the
// callee's free is the only one. It is the analogue of NAPI zeroing its
// capacity marker.
class RustOwnedBuffer : public jsi::MutableBuffer {
public:
  RustOwnedBuffer(UbrnJsiModule *m, UbrnRustBuffer rb) : m_(m), rb_(rb) {}
  ~RustOwnedBuffer() override {
    if (owns_) {
      ubrn_jsi_rustbuffer_free(m_, rb_);
    }
  }
  size_t size() const override { return (size_t)rb_.len; }
  uint8_t *data() override { return rb_.data; }

  bool owns() const { return owns_; }
  // Hand the allocation to a callee; the destructor becomes a no-op.
  UbrnRustBuffer release() {
    owns_ = false;
    return rb_;
  }

private:
  UbrnJsiModule *m_;
  UbrnRustBuffer rb_;
  bool owns_ = true;
};

// Recovers the owner from a view's ArrayBuffer.
//
// Attached to the ArrayBuffer rather than the Uint8Array deliberately: a
// `subarray` shares the ArrayBuffer but is a fresh Uint8Array, so a marker on
// the view would be lost exactly where it matters — the string lowering path
// shrinks its view to the bytes actually written before passing it.
class RustBufferOwner : public jsi::NativeState {
public:
  explicit RustBufferOwner(std::shared_ptr<RustOwnedBuffer> b)
      : buffer(std::move(b)) {}
  std::shared_ptr<RustOwnedBuffer> buffer;
};

// Produce the RustBuffer for a JS value being lowered into an FFI argument,
// adopting the allocation when the view already owns one.
inline UbrnRustBuffer rustBufferForArg(jsi::Runtime &rt, UbrnJsiModule *module,
                                       const jsi::Value &v) {
  auto obj = v.asObject(rt);
  jsi::ArrayBuffer ab =
      obj.isArrayBuffer(rt)
          ? obj.getArrayBuffer(rt)
          : obj.getPropertyAsObject(rt, "buffer").getArrayBuffer(rt);
  size_t byteOffset = 0, byteLength = ab.size(rt);
  if (!obj.isArrayBuffer(rt)) {
    byteOffset = (size_t)obj.getProperty(rt, "byteOffset").asNumber();
    byteLength = (size_t)obj.getProperty(rt, "byteLength").asNumber();
  }

  // Only a view starting at the allocation can be adopted: the callee frees
  // from the pointer it is given, so that pointer has to be the one the
  // allocator handed out. An offset view is copied instead.
  if (byteOffset == 0 && ab.hasNativeState(rt)) {
    auto owner =
        std::dynamic_pointer_cast<RustBufferOwner>(ab.getNativeState(rt));
    if (owner && owner->buffer) {
      if (!owner->buffer->owns()) {
        // Already adopted, so the callee has since freed it. Reading it now
        // would hand out a dangling pointer.
        throw jsi::JSError(rt, "uniffi jsi player: RustBuffer argument was "
                               "already consumed by a previous FFI call");
      }
      UbrnRustBuffer adopted = owner->buffer->release();
      // Capacity stays as allocated so the callee frees the whole region; len
      // narrows to the bytes the caller actually filled.
      adopted.len = (uint64_t)byteLength;
      return adopted;
    }
  }

  return ubrn_jsi_rustbuffer_from_bytes(module, ab.data(rt) + byteOffset,
                                        byteLength);
}
