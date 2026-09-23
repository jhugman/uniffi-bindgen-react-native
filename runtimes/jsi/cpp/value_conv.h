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
#include <string_view>
#include <utility>
#include <vector>

#include "ubrn_jsi.h"

namespace jsi = facebook::jsi;

// UbrnRustBuffer must match core::RustBufferC exactly; abi_assert.cpp pins
// that layout for every ABI, and is compiled into every build of this library.

// The shim's own marshalling discriminant. Not an ABI type: only tag NAMES
// cross into core. Kept numeric because every per-call marshal switches on it.
typedef enum {
  UBRN_TY_VOID = 0,
  UBRN_TY_U8 = 1,  UBRN_TY_I8 = 2,
  UBRN_TY_U16 = 3, UBRN_TY_I16 = 4,
  UBRN_TY_U32 = 5, UBRN_TY_I32 = 6,
  UBRN_TY_U64 = 7, UBRN_TY_I64 = 8,
  UBRN_TY_F32 = 9, UBRN_TY_F64 = 10,
  UBRN_TY_HANDLE = 11,
  UBRN_TY_RUSTBUFFER = 12, // fully supported (args and return values)
  UBRN_TY_CALLBACK = 13,   // named; the name travels in the parallel *_type_names array
  UBRN_TY_STRUCT = 14,     // named; the name travels in the parallel *_type_names array
  UBRN_TY_REFERENCE = 15,  // pointer to a named struct (vtable); marshalled as
                           // Reference(Struct(name))
  UBRN_TY_RUSTCALLSTATUS = 16, // inline RustCallStatus struct ({i8, u64, u64, ptr});
                               // appears only as a struct field (e.g. inside
                               // ForeignFutureResult<T>)
  // Not a type. What ubrnTagFromName answers with for a name the shim cannot
  // marshal; also marks a signature the player skips.
  UBRN_TY_UNSUPPORTED = 0xFF,
} UbrnFfiType;

// Player tag name -> the shim's discriminant. These are core's wire names; a
// name absent here is one the shim cannot marshal, and the whole function is
// skipped at registration. Called once per argument at registration, so a scan
// over 17 entries is the right shape.
inline UbrnFfiType ubrnTagFromName(std::string_view n) {
  struct Entry {
    std::string_view name;
    UbrnFfiType tag;
  };
  static constexpr Entry kTable[] = {
      {"Void", UBRN_TY_VOID},
      {"UInt8", UBRN_TY_U8},     {"Int8", UBRN_TY_I8},
      {"UInt16", UBRN_TY_U16},   {"Int16", UBRN_TY_I16},
      {"UInt32", UBRN_TY_U32},   {"Int32", UBRN_TY_I32},
      {"UInt64", UBRN_TY_U64},   {"Int64", UBRN_TY_I64},
      {"Float32", UBRN_TY_F32},  {"Float64", UBRN_TY_F64},
      {"Handle", UBRN_TY_HANDLE},
      {"RustBuffer", UBRN_TY_RUSTBUFFER},
      {"Callback", UBRN_TY_CALLBACK},
      {"Struct", UBRN_TY_STRUCT},
      {"Reference", UBRN_TY_REFERENCE},
      {"RustCallStatus", UBRN_TY_RUSTCALLSTATUS},
  };
  for (const auto &e : kTable) {
    if (e.name == n) {
      return e.tag;
    }
  }
  return UBRN_TY_UNSUPPORTED;
}

// One argument's type, carrying the name for Callback/Struct/Reference tags.
// For scalars/RustBuffer `name` is empty. `tag` is a UbrnFfiType (or
// UBRN_TY_UNSUPPORTED). `size` is core's byte width for this type's flat arg
// slot, resolved once at registration so no call path crosses the ABI for it.
struct ArgDesc {
  uint8_t tag = UBRN_TY_VOID;
  std::string name; // callback/struct name for named tags, else empty
  size_t size = 0;  // core's slot width, or 0 where core gives none
  // The player tag name ("UInt8", "Callback", …) as codegen emits it, after the
  // shim's own normalisations below. This is what crosses the ABI; `tag` is the
  // shim's switch discriminant. The two always describe the same type.
  std::string tagName;
};

// Classify a DEFINITIONS `{ tag, name?, inner? }` object into an ArgDesc's
// tag/name. Leaves `size` at 0 — argDescFromDefObject fills that in. The shim
// owns the name->tag table (ubrnTagFromName); what stays here is the two names
// core has no number for and the per-tag name/inner lookups off the object.
//   Callback  -> (UBRN_TY_CALLBACK, name)
//   Struct    -> (UBRN_TY_STRUCT, name)
//   Reference(Struct(name)) -> (UBRN_TY_REFERENCE, name)  (vtable pointer arg)
// Scalars/RustBuffer carry an empty name. Anything else -> UBRN_TY_UNSUPPORTED,
// which skips the whole function at registration.
inline ArgDesc argDescTypeFromDefObject(jsi::Runtime &rt,
                                        const jsi::Object &o) {
  auto tagName = o.getProperty(rt, "tag").asString(rt).utf8(rt);
  ArgDesc d;
  // "MutReference" has no tag of its own; the player marshals it exactly as
  // "Reference" — a pointer, checked for a Struct target below.
  if (tagName == "MutReference")
    tagName = "Reference";
  d.tag = ubrnTagFromName(tagName);
  // A VoidPointer is pointer-sized; we treat it like a Handle for byte layout.
  // A shim-local parsing choice, not an ABI tag. (Appears as a callback's
  // out_return arg type / void return; never needs a name.)
  if (d.tag == UBRN_TY_UNSUPPORTED && tagName == "VoidPointer") {
    d.tag = UBRN_TY_HANDLE;
    tagName = "Handle";
  }
  d.tagName = tagName;

  switch (d.tag) {
  case UBRN_TY_CALLBACK:
  case UBRN_TY_STRUCT:
    d.name = o.getProperty(rt, "name").asString(rt).utf8(rt);
    break;
  case UBRN_TY_REFERENCE: {
    // Only a pointer to a named struct (a vtable) is marshallable; a reference
    // to anything else the player skips.
    auto inner = o.getProperty(rt, "inner").asObject(rt);
    auto innerTag = inner.getProperty(rt, "tag").asString(rt).utf8(rt);
    if (ubrnTagFromName(innerTag) == UBRN_TY_STRUCT) {
      d.name = inner.getProperty(rt, "name").asString(rt).utf8(rt);
    } else {
      d.tag = UBRN_TY_UNSUPPORTED;
    }
    break;
  }
  default:
    break;
  }
  return d;
}

// Classify a DEFINITIONS type object AND resolve its flat-slot byte width from
// core's geometry table, so the shim never derives a size of its own. Called at
// registration; every call path then reads ArgDesc::size.
inline ArgDesc argDescFromDefObject(jsi::Runtime &rt, const jsi::Object &o) {
  ArgDesc d = argDescTypeFromDefObject(rt, o);
  // Core gives no geometry for a bare Struct (it only travels behind a
  // pointer), and an unsupported desc must not carry one either — a
  // Reference to a non-Struct keeps the marshallable tag name, so ask only
  // for tags the shim can actually marshal. Both keep size 0, and every call
  // path that would read a width intercepts them before reaching it.
  size_t size = 0;
  if (d.tag != UBRN_TY_UNSUPPORTED &&
      ubrn_jsi_scalar_slot_size_align(d.tagName.c_str(), &size, nullptr))
    d.size = size;
  return d;
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

// Write a jsi number/bool/bigint into `dst` (the tag's slot width) as native
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
