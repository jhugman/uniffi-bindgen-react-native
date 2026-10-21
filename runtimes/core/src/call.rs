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
//! 2. **Buffer** ([`PreparedCall`]) — a zeroed byte buffer sized for one call,
//!    inline for ordinary signatures and heap-backed only for wide ones, with
//!    accessor methods that hand out correctly-sized mutable slices per argument.
//! 3. **Invocation** ([`Module::call`]) — builds libffi `Arg` references from
//!    the buffer, calls the resolved symbol, and writes the return value's
//!    native-endian bytes into a caller-provided buffer.
//!
//! The bridge layer (e.g. napi) is responsible for converting JS values into
//! the bytes that fill each slot, and for interpreting the written return
//! bytes back into JS values. Core never touches JS types.

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

/// Byte width of a function's return value as [`Module::call`] writes it.
///
/// Differs from [`slot_size_align`] for pointer returns, which `invoke`
/// widens to 8 bytes so the width does not vary with the host's pointer size.
/// Bridges size the return buffer from this so the width the writer uses and
/// the width the reader allocates come from one place; sizing from the slot
/// geometry gives pointer width, which is too small on a 32-bit host.
///
/// Not for callback returns: a trampoline's return is written by libffi at
/// the CIF's own width, which `Module::callback_return_size` reports.
pub fn return_size(desc: &FfiTypeDesc) -> Result<usize> {
    match desc {
        FfiTypeDesc::Void => Ok(0),
        FfiTypeDesc::VoidPointer
        | FfiTypeDesc::Reference(_)
        | FfiTypeDesc::MutReference(_)
        | FfiTypeDesc::Callback(_) => Ok(size_of::<u64>()),
        other => slot_size_align(other).map(|(size, _)| size),
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
    bytes: ArgBytes,
}

/// Inline capacity of [`ArgBytes`]: eight 8-byte slots, enough for uniffi's
/// ordinary signatures plus the trailing `RustCallStatus` pointer.
const INLINE_ARG_BYTES: usize = 64;

/// Slot offsets are aligned relative to the buffer start, so the start must be
/// at least as aligned as the widest slot.
#[repr(C, align(16))]
struct InlineArgs([u8; INLINE_ARG_BYTES]);

/// A call's argument buffer. Every FFI crossing needs one, so ordinary
/// signatures use inline storage and only wide ones reach the heap.
enum ArgBytes {
    Inline { buf: InlineArgs, len: usize },
    Heap(Vec<u8>),
}

impl ArgBytes {
    fn zeroed(len: usize) -> Self {
        if len <= INLINE_ARG_BYTES {
            Self::Inline {
                buf: InlineArgs([0u8; INLINE_ARG_BYTES]),
                len,
            }
        } else {
            Self::Heap(vec![0u8; len])
        }
    }

    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Inline { buf, len } => &buf.0[..*len],
            Self::Heap(v) => v,
        }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        match self {
            Self::Inline { buf, len } => &mut buf.0[..*len],
            Self::Heap(v) => v,
        }
    }
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
        Ok(&mut self.bytes.as_mut_slice()[slot.offset..slot.offset + slot.size])
    }

    /// Return a mutable slice for the trailing `*mut RustCallStatus` slot,
    /// or `None` if this function doesn't use one.
    pub fn rust_call_status_slot(&mut self) -> Option<&mut [u8]> {
        let slot = self.function.arg_layout.rust_call_status_slot.as_ref()?;
        Some(&mut self.bytes.as_mut_slice()[slot.offset..slot.offset + slot.size])
    }

    /// Consume the buffer, invoke the resolved function, and write the return
    /// value's native-endian bytes into `out`.
    pub(crate) fn invoke(self, out: &mut [u8]) -> Result<usize> {
        self.function.invoke(self.bytes.as_slice(), out)
    }
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
            bytes: ArgBytes::zeroed(function.arg_layout.total_size),
        })
    }

    /// Invoke a [`PreparedCall`], writing the return value's native-endian bytes
    /// into `out`. Returns the number of bytes written (0 for a void return).
    ///
    /// Guards the call with lifecycle checks: returns `Err(Unloading)` if the
    /// module is shutting down. The `PreparedCall` is consumed.
    pub fn call(&self, args: PreparedCall<'_>, out: &mut [u8]) -> Result<usize> {
        if !self.lifecycle.try_begin_call() {
            return Err(Error::Unloading);
        }
        let result = args.invoke(out);
        self.lifecycle.end_call();
        result
    }

    /// Allocate a Rust-owned buffer of `size` bytes via the library's `rustbuffer_alloc`.
    pub fn rustbuffer_alloc(&self, size: i32) -> Result<RustBufferC> {
        use crate::ffi_c_types::{RustBufferAllocFn, RustCallStatusC};
        if !self.lifecycle.try_begin_call() {
            return Err(Error::Unloading);
        }
        // SAFETY: `alloc_ptr` is the address dlsym returned at registration for the
        // name the spec gave as `rustbuffer_alloc`, and the caller of `Module::new`
        // warrants that name denotes a UniFFI-generated `rustbuffer_alloc`, whose
        // signature is `RustBufferAllocFn`. The library it came from is kept open
        // for the Module's lifetime.
        let func: RustBufferAllocFn = unsafe { std::mem::transmute(self.rb_ops.alloc_ptr) };
        let mut status = RustCallStatusC::default();
        // SAFETY: `func` has the signature transmuted above; `status` is a live
        // local the callee may write its error out-param through.
        let rb = unsafe { func(size, &mut status) };
        self.lifecycle.end_call();
        if status.code != 0 {
            return Err(Error::Other(format!(
                "rustbuffer_alloc failed: status code {}",
                status.code
            )));
        }
        Ok(rb)
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
    use std::collections::HashMap;
    use std::sync::Arc;

    use crate::module::AbortCallbacksFn;

    use crate::spec::{FunctionDef, ModuleSpec};
    use crate::test_support::{
        fixture_cdylib_path, hello_world_rustbuffer_symbols, noop_abort_callbacks,
    };

    /// The width `invoke` writes, per return type: 8 for every pointer kind on
    /// every host, the slot width otherwise, and none for a bare Struct.
    #[test]
    fn return_size_widens_pointer_returns() {
        for desc in [
            FfiTypeDesc::VoidPointer,
            FfiTypeDesc::Reference(Box::new(FfiTypeDesc::Struct("S".into()))),
            FfiTypeDesc::MutReference(Box::new(FfiTypeDesc::Struct("S".into()))),
            FfiTypeDesc::Callback("cb".into()),
        ] {
            assert_eq!(return_size(&desc).unwrap(), 8, "{desc:?}");
        }
        assert_eq!(return_size(&FfiTypeDesc::Void).unwrap(), 0);
        assert_eq!(return_size(&FfiTypeDesc::UInt16).unwrap(), 2);
        assert_eq!(
            return_size(&FfiTypeDesc::RustBuffer).unwrap(),
            size_of::<RustBufferC>()
        );
        assert!(return_size(&FfiTypeDesc::Struct("S".into())).is_err());
    }

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

    #[test]
    fn arg_bytes_inline_up_to_capacity_then_heap() {
        for len in [0, 1, INLINE_ARG_BYTES] {
            let bytes = ArgBytes::zeroed(len);
            assert!(matches!(bytes, ArgBytes::Inline { .. }), "len {len}");
            assert_eq!(bytes.as_slice(), vec![0u8; len]);
        }
        let len = INLINE_ARG_BYTES + 1;
        let bytes = ArgBytes::zeroed(len);
        assert!(matches!(bytes, ArgBytes::Heap(_)));
        assert_eq!(bytes.as_slice(), vec![0u8; len]);
    }

    /// Slot offsets are relative to the buffer start, so an unaligned start
    /// would misalign every 8-byte slot.
    #[test]
    fn inline_args_are_aligned_for_every_slot() {
        let widest = [
            align_of::<u64>(),
            align_of::<f64>(),
            align_of::<*const c_void>(),
            align_of::<RustBufferC>(),
        ]
        .into_iter()
        .max()
        .unwrap();
        assert!(align_of::<InlineArgs>() >= widest);
    }

    /// Build the `hello-world` fixture cdylib and load it as a `Module` wired only
    /// with the RustBuffer symbols (no functions/callbacks/structs) — enough for
    /// the rustbuffer_* guard tests, which don't need a real function call.
    fn test_module() -> Arc<Module> {
        let spec = ModuleSpec {
            rustbuffer_symbols: hello_world_rustbuffer_symbols(),
            functions: Default::default(),
            callbacks: Default::default(),
            structs: Default::default(),
        };
        let abort: AbortCallbacksFn = noop_abort_callbacks;
        Module::new(&fixture_cdylib_path(), spec, abort, std::ptr::null()).expect("module load")
    }

    /// The `hello-world` fixture with `add(u32, u32) -> u32` registered.
    fn test_module_with_add() -> Arc<Module> {
        let mut functions = HashMap::new();
        functions.insert(
            "uniffi_hello_world_fn_func_add".to_string(),
            FunctionDef {
                args: vec![FfiTypeDesc::UInt32, FfiTypeDesc::UInt32],
                ret: FfiTypeDesc::UInt32,
                has_rust_call_status: true,
            },
        );
        let spec = ModuleSpec {
            rustbuffer_symbols: hello_world_rustbuffer_symbols(),
            functions,
            callbacks: Default::default(),
            structs: Default::default(),
        };
        let abort: AbortCallbacksFn = noop_abort_callbacks;
        Module::new(&fixture_cdylib_path(), spec, abort, std::ptr::null()).expect("module load")
    }

    #[test]
    fn rustbuffer_alloc_refuses_after_unload() {
        let m = test_module();
        m.unload().expect("unload");
        assert!(matches!(m.rustbuffer_alloc(16), Err(Error::Unloading)));
    }

    #[test]
    fn call_writes_native_endian_return_bytes() {
        use crate::ffi_c_types::RustCallStatusC;

        let m = test_module_with_add();
        let mut call = m
            .prepare_call("uniffi_hello_world_fn_func_add")
            .expect("prepare_call");
        call.arg_slot(0)
            .expect("arg 0")
            .copy_from_slice(&20u32.to_ne_bytes());
        call.arg_slot(1)
            .expect("arg 1")
            .copy_from_slice(&22u32.to_ne_bytes());
        // The callee's signature takes `&mut RustCallStatus`; it must point
        // somewhere real even though this call can't fail.
        let mut status = RustCallStatusC::default();
        if let Some(slot) = call.rust_call_status_slot() {
            crate::slot::write_pointer(slot, &mut status as *mut RustCallStatusC as *const c_void);
        }
        let mut out = [0u8; 8];
        let n = m.call(call, &mut out).expect("call");
        assert_eq!(n, 4, "u32 return is 4 bytes");
        assert_eq!(u32::from_ne_bytes(out[..4].try_into().unwrap()), 42);
    }

    #[test]
    fn call_rejects_a_return_buffer_that_is_too_small() {
        use crate::ffi_c_types::RustCallStatusC;

        let m = test_module_with_add();
        let mut call = m
            .prepare_call("uniffi_hello_world_fn_func_add")
            .expect("prepare_call");
        call.arg_slot(0)
            .expect("arg 0")
            .copy_from_slice(&1u32.to_ne_bytes());
        call.arg_slot(1)
            .expect("arg 1")
            .copy_from_slice(&1u32.to_ne_bytes());
        // Same as above: give the callee a real status slot to write through.
        let mut status = RustCallStatusC::default();
        if let Some(slot) = call.rust_call_status_slot() {
            crate::slot::write_pointer(slot, &mut status as *mut RustCallStatusC as *const c_void);
        }
        let mut out = [0u8; 2];
        assert!(m.call(call, &mut out).is_err());
    }
}
