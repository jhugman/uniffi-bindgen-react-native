/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Argument layout, buffer management, and FFI call dispatch.
//!
//! This module owns the data-flow path from the runtime bridge into the loaded
//! native library:
//!
//! 1. **Layout** ([`ArgLayout`] / [`SlotLayout`]) — precomputed byte offsets
//!    and sizes for each argument, so the bridge layer can write values directly
//!    into a flat buffer without per-call allocation.
//! 2. **Buffer** ([`PreparedCall`]) — a zeroed byte vec sized for one call, with
//!    accessor methods that hand out correctly-sized mutable slices per argument.
//! 3. **Invocation** ([`invoke`]) — builds libffi `Arg` references from the
//!    buffer and calls the resolved symbol, returning a typed [`CallReturn`].
//!
//! The bridge layer (e.g. napi) is responsible for converting JS values into
//! the bytes that fill each slot, and for interpreting `CallReturn` variants
//! back into JS values. Core never touches JS types.

use std::ffi::c_void;
use std::mem::{align_of, size_of};

use libffi::low::CodePtr;

use crate::ffi_c_types::RustBufferC;
use crate::module::ResolvedFunction;
use crate::{Error, FfiTypeDesc, Result};

/// Byte offset and size of a single argument within an [`PreparedCall`].
#[derive(Debug, Clone)]
pub struct SlotLayout {
    pub offset: usize,
    pub size: usize,
}

/// Precomputed layout for one function's args + optional RustCallStatus.
#[derive(Debug, Clone)]
pub struct ArgLayout {
    pub arg_slots: Vec<SlotLayout>,
    pub rust_call_status_slot: Option<SlotLayout>,
    pub total_size: usize,
}

/// Return the `(size, alignment)` pair for one argument slot of the given FFI type.
///
/// This is the source of truth that [`ArgLayout::compute`] uses when packing
/// argument buffers, and that the bridge layer can use when it needs to know
/// how many bytes a slot occupies (e.g. for struct field offsets).
pub fn slot_size_align(desc: &FfiTypeDesc) -> Result<(usize, usize)> {
    match desc {
        FfiTypeDesc::UInt8 | FfiTypeDesc::Int8 => Ok((size_of::<u8>(), align_of::<u8>())),
        FfiTypeDesc::UInt16 | FfiTypeDesc::Int16 => Ok((size_of::<u16>(), align_of::<u16>())),
        FfiTypeDesc::UInt32 | FfiTypeDesc::Int32 => Ok((size_of::<u32>(), align_of::<u32>())),
        FfiTypeDesc::UInt64 | FfiTypeDesc::Int64 | FfiTypeDesc::Handle => {
            Ok((size_of::<u64>(), align_of::<u64>()))
        }
        FfiTypeDesc::Float32 => Ok((size_of::<f32>(), align_of::<f32>())),
        FfiTypeDesc::Float64 => Ok((size_of::<f64>(), align_of::<f64>())),
        FfiTypeDesc::RustBuffer => Ok((size_of::<RustBufferC>(), align_of::<RustBufferC>())),
        FfiTypeDesc::VoidPointer
        | FfiTypeDesc::Reference(_)
        | FfiTypeDesc::MutReference(_)
        | FfiTypeDesc::Callback(_) => Ok((size_of::<*const c_void>(), align_of::<*const c_void>())),
        FfiTypeDesc::RustCallStatus => Ok((size_of::<*mut c_void>(), align_of::<*mut c_void>())),
        FfiTypeDesc::Void => Ok((0, 1)),
        FfiTypeDesc::Struct(_) | FfiTypeDesc::ForeignBytes => Err(Error::UnsupportedType(format!(
            "{desc:?} is not allowed as an arg slot (structs go through a pointer arg)"
        ))),
    }
}

impl ArgLayout {
    /// Walk the argument list and compute a packed layout with correct alignment
    /// for each slot. If `has_rust_call_status` is true, a pointer-sized slot is
    /// appended for the `*mut RustCallStatus` out-parameter.
    pub fn compute(args: &[FfiTypeDesc], has_rust_call_status: bool) -> Result<Self> {
        let mut offset = 0usize;
        let mut arg_slots = Vec::with_capacity(args.len());
        for desc in args {
            let (size, align) = slot_size_align(desc)?;
            offset = (offset + align - 1) & !(align - 1);
            arg_slots.push(SlotLayout { offset, size });
            offset += size;
        }
        let rust_call_status_slot = if has_rust_call_status {
            let (size, align) = (size_of::<*mut c_void>(), align_of::<*mut c_void>());
            offset = (offset + align - 1) & !(align - 1);
            let slot = SlotLayout { offset, size };
            offset += size;
            Some(slot)
        } else {
            None
        };
        Ok(ArgLayout {
            arg_slots,
            rust_call_status_slot,
            total_size: offset,
        })
    }
}

/// A ready-to-fill argument buffer for one function call.
///
/// Created via [`Module::prepare_call`]. The bridge layer fills each slot
/// using [`arg_slot`](Self::arg_slot) (for regular arguments) and
/// [`rust_call_status_slot`](Self::rust_call_status_slot) (for the trailing
/// error out-parameter), then passes the buffer to [`Module::call`].
pub struct PreparedCall<'m> {
    function: &'m ResolvedFunction,
    bytes: Vec<u8>,
}

impl<'m> PreparedCall<'m> {
    /// Return a mutable slice for the `idx`-th argument slot.
    ///
    /// The slice is exactly `size` bytes as determined by the [`ArgLayout`],
    /// ready for the bridge layer to write a native-endian value into.
    pub fn arg_slot(&mut self, idx: usize) -> Result<&mut [u8]> {
        let slot = self
            .function
            .arg_layout
            .arg_slots
            .get(idx)
            .ok_or_else(|| Error::Other(format!("arg slot {idx} out of range")))?;
        Ok(&mut self.bytes[slot.offset..slot.offset + slot.size])
    }

    /// Return a mutable slice for the trailing `*mut RustCallStatus` slot,
    /// or `None` if this function doesn't use one.
    pub fn rust_call_status_slot(&mut self) -> Option<&mut [u8]> {
        let slot = self.function.arg_layout.rust_call_status_slot.as_ref()?;
        Some(&mut self.bytes[slot.offset..slot.offset + slot.size])
    }

    /// Consume the buffer and invoke the resolved function.
    pub(crate) fn invoke(self) -> Result<CallReturn> {
        self.function.invoke(&self.bytes)
    }
}

/// The typed return value from an FFI call.
///
/// Each variant carries the Rust-native type that libffi produced. The bridge
/// layer pattern-matches on this to create the appropriate JS representation
/// (e.g. `U32` -> `env.create_uint32()`, `I64` -> `env.create_bigint_from_i64()`).
///
/// Byte-level serialisation is deliberately *not* done here — that's the
/// bridge layer's responsibility.
#[derive(Debug)]
pub enum CallReturn {
    Void,
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    U64(u64),
    I64(i64),
    F32(f32),
    F64(f64),
    /// A raw pointer return (void*, references, callback fn pointers).
    Pointer(usize),
    /// An owned `RustBuffer` that the bridge layer must eventually free.
    RustBuffer(RustBufferC),
}

// ---------------------------------------------------------------------------
// impl Module: JS -> Rust (frontend calls into the loaded library)
// ---------------------------------------------------------------------------

use crate::module::Module;

impl Module {
    /// Create a zeroed [`PreparedCall`] for the named function.
    pub fn prepare_call(&self, fn_name: &str) -> Result<PreparedCall<'_>> {
        let function = self
            .functions
            .get(fn_name)
            .ok_or_else(|| Error::UnknownFunction(fn_name.to_string()))?;
        Ok(PreparedCall {
            function,
            bytes: vec![0u8; function.arg_layout.total_size],
        })
    }

    /// Invoke a [`PreparedCall`] whose argument slots have been filled by the
    /// bridge layer.
    ///
    /// Guards the call with lifecycle checks: returns `Err(Unloading)` if the
    /// module is shutting down. The `PreparedCall` is consumed.
    pub fn call(&self, args: PreparedCall<'_>) -> Result<CallReturn> {
        if !self.lifecycle.try_begin_call() {
            return Err(Error::Unloading);
        }
        let result = args.invoke();
        self.lifecycle.end_call();
        result
    }

    /// Copy JS-owned bytes into a new Rust-allocated `RustBufferC`.
    pub fn rustbuffer_from_bytes(&self, data: *const u8, len: usize) -> Result<RustBufferC> {
        use crate::ffi_c_types::{ForeignBytesC, RustBufferFromBytesFn, RustCallStatusC};
        if !self.lifecycle.try_begin_call() {
            return Err(Error::Unloading);
        }
        // SAFETY: from_bytes_ptr was resolved via dlsym and transmuted to the correct
        // fn signature. data/len are caller-guaranteed valid. status is stack-allocated.
        let result = unsafe {
            let func: RustBufferFromBytesFn = std::mem::transmute(self.rb_ops.from_bytes_ptr);
            let mut status = RustCallStatusC::default();
            let foreign = ForeignBytesC {
                len: len as i32,
                data,
            };
            let rb = func(foreign, &mut status);
            if status.code != 0 {
                self.lifecycle.end_call();
                return Err(Error::Other(format!(
                    "rustbuffer_from_bytes failed: status code {}",
                    status.code
                )));
            }
            rb
        };
        self.lifecycle.end_call();
        Ok(result)
    }

    /// Free a Rust-allocated `RustBufferC`.
    pub fn rustbuffer_free(&self, rb: RustBufferC) -> Result<()> {
        use crate::ffi_c_types::{RustBufferFreeFn, RustCallStatusC};
        if !self.lifecycle.try_begin_call() {
            return Err(Error::Unloading);
        }
        // SAFETY: free_ptr was resolved via dlsym; rb was allocated by the same library.
        unsafe {
            let func: RustBufferFreeFn = std::mem::transmute(self.rb_ops.free_ptr);
            let mut status = RustCallStatusC::default();
            func(rb, &mut status);
        }
        self.lifecycle.end_call();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// impl Module: JS -> C fn ptr (frontend invokes a callback-shaped fn pointer)
// ---------------------------------------------------------------------------

impl Module {
    /// Invoke a raw C function pointer using the signature of the named callback.
    ///
    /// Each entry in `arg_buffers` holds the raw bytes for one argument:
    /// scalars (1/2/4/8 bytes), RustBuffer (24 bytes), structs (full C struct bytes).
    pub fn call_callback_ptr(
        &self,
        callback_name: &str,
        fn_ptr: *const c_void,
        arg_buffers: Vec<Vec<u8>>,
    ) -> Result<()> {
        let cif = self
            .callback_cifs
            .get(callback_name)
            .ok_or_else(|| Error::UnknownCallback(callback_name.to_string()))?;

        let def = self
            .spec
            .callbacks
            .get(callback_name)
            .ok_or_else(|| Error::UnknownCallback(callback_name.to_string()))?;

        let mut ffi_args: Vec<libffi::middle::Arg> = Vec::with_capacity(arg_buffers.len());
        for buf in &arg_buffers {
            // SAFETY: buf.as_ptr() points to a valid byte buffer that outlives the cif.call below.
            ffi_args.push(unsafe { libffi::middle::arg(&*(buf.as_ptr() as *const c_void)) });
        }

        let code_ptr = CodePtr::from_ptr(fn_ptr);

        match &def.ret {
            FfiTypeDesc::Void => {
                // SAFETY: CIF matches the callback def; args are correctly marshalled byte buffers;
                // fn_ptr is a valid C function pointer from the loaded library.
                unsafe { cif.call::<()>(code_ptr, &ffi_args) };
                Ok(())
            }
            other => Err(Error::UnsupportedType(format!(
                "call_callback_ptr: non-void return type {other:?} not yet supported"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_int32_int64() {
        let lay = ArgLayout::compute(&[FfiTypeDesc::Int32, FfiTypeDesc::Int64], false).unwrap();
        assert_eq!(lay.arg_slots[0].offset, 0);
        assert_eq!(lay.arg_slots[0].size, 4);
        assert_eq!(lay.arg_slots[1].offset, 8); // aligned to 8
        assert_eq!(lay.arg_slots[1].size, 8);
        assert_eq!(lay.total_size, 16);
        assert!(lay.rust_call_status_slot.is_none());
    }

    #[test]
    fn layout_with_rust_call_status() {
        let lay = ArgLayout::compute(&[FfiTypeDesc::Int32], true).unwrap();
        assert!(lay.rust_call_status_slot.is_some());
    }
}
