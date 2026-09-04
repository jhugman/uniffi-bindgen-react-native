/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Module: one loaded library with resolved symbols, CIFs, and lifecycle state.
//!
//! The public API is organized by data-flow direction:
//!
//! - **JS -> Rust** (`call`, `prepare_call`, `rustbuffer_*`):
//!   Frontend invokes functions exported by the loaded Rust library.
//!
//! - **Rust -> JS** (`make_callback_trampoline`, `build_vtable`—in `callback.rs`):
//!   The loaded library invokes callbacks/VTable methods implemented by the frontend.
//!
//! - **JS -> C fn ptr** (`call_callback_ptr`):
//!   Frontend invokes a raw C function pointer whose signature matches a named callback
//!   (e.g. `ForeignFutureComplete*` completion handlers passed from Rust to JS).

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use libffi::low::CodePtr;
use libffi::middle::Cif;

use crate::call::{slot_size_align, ArgLayout};
use crate::cif::ffi_type_for;
use crate::ffi_c_types::{RustBufferC, RustBufferOps};
use crate::ffi_type::desc_from_name;
use crate::library::LibraryHandle;
use crate::spec::{CallbackDef, FunctionDef, ModuleSpec, StructDef};
use crate::{Error, FfiTypeDesc, Result};

/// A function that has been resolved against a loaded library and pre-flighted with a CIF.
///
/// All fields are immutable after construction. The [`invoke`](Self::invoke)
/// method builds a stack-local `Arg` array from a filled byte buffer and
/// dispatches through the pre-built CIF — the `Arg` pointers cannot escape.
pub(crate) struct ResolvedFunction {
    def: FunctionDef,
    symbol: *const c_void,
    cif: Cif,
    pub(crate) arg_layout: ArgLayout,
}

impl ResolvedFunction {
    /// Build libffi `Arg` references into `arg_bytes`, one per CIF argument.
    ///
    /// `invoke` dispatches through this, so the slot-offset reasoning lives in
    /// one place. The returned `Arg`s borrow `arg_bytes` and must not outlive
    /// the `cif.call` they are handed to.
    ///
    /// `arg_bytes` must be at least `self.arg_layout.total_size` bytes; it is
    /// always the [`PreparedCall`](crate::PreparedCall) buffer, which
    /// `prepare_call` sizes from this very layout.
    fn ffi_args<'a>(&self, arg_bytes: &'a [u8]) -> Vec<libffi::middle::Arg<'a>> {
        let buf_ptr = arg_bytes.as_ptr();

        let mut ffi_args: Vec<libffi::middle::Arg<'a>> =
            Vec::with_capacity(self.arg_layout.arg_slots.len() + 1);
        for slot in &self.arg_layout.arg_slots {
            // SAFETY: ArgLayout::compute bounds slot.offset + slot.size by total_size,
            // and arg_bytes is at least that long (see this fn's doc comment).
            let slot_ptr = unsafe { buf_ptr.add(slot.offset) };
            // SAFETY: slot_ptr points into arg_bytes, which outlives the returned Arg.
            ffi_args.push(unsafe { libffi::middle::arg(&*slot_ptr) });
        }
        if let Some(rcs_slot) = &self.arg_layout.rust_call_status_slot {
            // SAFETY: rcs_slot.offset is within arg_bytes, same reasoning as above.
            let slot_ptr = unsafe { buf_ptr.add(rcs_slot.offset) };
            // SAFETY: slot_ptr points into arg_bytes, same reasoning as above.
            ffi_args.push(unsafe { libffi::middle::arg(&*slot_ptr) });
        }
        ffi_args
    }

    /// Call the resolved symbol, writing the return value's native-endian bytes
    /// into `out`. Returns the number of bytes written (0 for a void return).
    ///
    /// A pointer return is widened to 8 bytes so the byte width does not vary
    /// with the host's pointer size. A `RustBuffer` return is written as its
    /// 24-byte `repr(C)` form and stays owned by the caller, who must free it —
    /// so `out` must be at least 24 bytes for a RustBuffer-returning function
    /// or the buffer's backing allocation is leaked along with the error.
    pub(crate) fn invoke(&self, arg_bytes: &[u8], out: &mut [u8]) -> Result<usize> {
        let ffi_args = self.ffi_args(arg_bytes);

        let code_ptr = CodePtr::from_ptr(self.symbol);
        let ffi_args = &ffi_args;

        // Each arm picks the Rust return type that `ffi_type_for` mapped `self.def.ret`
        // to when `Module::new` built the CIF, so the call's type parameter and the
        // CIF's return type always agree.
        macro_rules! call_and_put {
            ($ty:ty) => {{
                // SAFETY: The CIF was built from the same FunctionDef as the arg buffer
                // layout, and $ty is the Rust type of that CIF's return type. Each
                // ffi_arg points into arg_bytes (alive for this call). code_ptr is a
                // resolved symbol from a loaded library alive for the Module's lifetime.
                let v: $ty = unsafe { self.cif.call(code_ptr, ffi_args) };
                put_return_bytes(out, &v.to_ne_bytes())
            }};
        }

        match &self.def.ret {
            FfiTypeDesc::Void => {
                // SAFETY: as in `call_and_put!`, with `()` for the CIF's void return type.
                unsafe { self.cif.call::<()>(code_ptr, ffi_args) };
                Ok(0)
            }
            FfiTypeDesc::UInt8 => call_and_put!(u8),
            FfiTypeDesc::Int8 => call_and_put!(i8),
            FfiTypeDesc::UInt16 => call_and_put!(u16),
            FfiTypeDesc::Int16 => call_and_put!(i16),
            FfiTypeDesc::UInt32 => call_and_put!(u32),
            FfiTypeDesc::Int32 => call_and_put!(i32),
            FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => call_and_put!(u64),
            FfiTypeDesc::Int64 => call_and_put!(i64),
            FfiTypeDesc::Float32 => call_and_put!(f32),
            FfiTypeDesc::Float64 => call_and_put!(f64),
            FfiTypeDesc::RustBuffer => {
                // SAFETY: as in `call_and_put!`, with RustBufferC for the CIF's RustBuffer
                // return type — the same repr(C) struct `ffi_type_for` describes to libffi.
                let rb: RustBufferC = unsafe { self.cif.call(code_ptr, ffi_args) };
                put_return_bytes(out, &crate::slot::rust_buffer_to_bytes(&rb))
            }
            FfiTypeDesc::VoidPointer
            | FfiTypeDesc::Reference(_)
            | FfiTypeDesc::MutReference(_)
            | FfiTypeDesc::Callback(_) => {
                // SAFETY: as in `call_and_put!`, with usize for the CIF's pointer return type.
                let v: usize = unsafe { self.cif.call(code_ptr, ffi_args) };
                put_return_bytes(out, &(v as u64).to_ne_bytes())
            }
            other => Err(Error::UnsupportedType(format!(
                "return type {other:?} not yet supported"
            ))),
        }
    }
}

/// Copy a return value's bytes into the front of `out`, or fail if they don't fit.
fn put_return_bytes(out: &mut [u8], bytes: &[u8]) -> Result<usize> {
    if out.len() < bytes.len() {
        return Err(Error::Other(format!(
            "return buffer too small: need {}, have {}",
            bytes.len(),
            out.len()
        )));
    }
    out[..bytes.len()].copy_from_slice(bytes);
    Ok(bytes.len())
}

// SAFETY: ResolvedFunction is created on one thread and read-only thereafter.
// The raw pointer `symbol` is valid for the lifetime of the containing Module's LibraryHandle.
// Cif is not auto-Send/Sync due to internal raw pointers but is safe to share read-only.
unsafe impl Send for ResolvedFunction {}
// SAFETY: All fields are immutable after construction; see Send impl above.
unsafe impl Sync for ResolvedFunction {}

/// C-ABI signature for the frontend-provided hook that aborts engine-specific
/// callback resources (e.g. NAPI TSFNs) during unload.
pub type AbortCallbacksFn = extern "C" fn(user_data: *const c_void);

/// A loaded UniFFI library with resolved symbols, pre-built CIFs, and lifecycle state.
///
/// The `library` field is wrapped in `Mutex<Option<_>>` so that `unload_force` can
/// take ownership and close it. All symbol lookups happen at construction time, so
/// the hot path (call/rustbuffer ops) never touches the mutex.
pub struct Module {
    pub(crate) library: Mutex<Option<LibraryHandle>>,
    pub(crate) spec: ModuleSpec,
    pub(crate) functions: HashMap<String, ResolvedFunction>,
    pub(crate) callback_cifs: HashMap<String, Cif>,
    pub(crate) struct_layouts: HashMap<String, StructLayout>,
    pub(crate) rb_ops: RustBufferOps,
    pub(crate) abort_callbacks: AbortCallbacksFn,
    pub(crate) abort_user_data: *const c_void,
    pub(crate) lifecycle: crate::lifecycle::UnloadState,
    /// Trampoline reuse cache for an engine-minted JS function identity and the
    /// callback name it was marshalled under. Keyed identity-first so a lookup
    /// borrows the name rather than allocating one — this is read on every
    /// callback marshal. Stores `usize`, not a raw pointer, so the field carries
    /// no pointer of its own; the `Mutex` is what the impls below rest on.
    pub(crate) trampolines: Mutex<HashMap<u64, HashMap<String, usize>>>,
    /// Trampolines this module has built, counting those the frontend never
    /// remembered. Monotonic: a trampoline is leaked by design, so nothing here
    /// ever decreases and the count is a leak total, not a live-entry count.
    /// Reuse is the only thing bounding it, which is what makes it worth
    /// exposing — see [`Module::trampolines_built`].
    pub(crate) trampolines_built: AtomicU64,
}

// SAFETY: interior mutability is via atomics in UnloadState and via Mutex (library,
// trampolines); ResolvedFunction is immutable after construction. Raw pointers
// (abort_user_data, rb_ops) are stable for the Module lifetime.
unsafe impl Send for Module {}
// SAFETY: Mutex guards library and trampolines; all other fields are immutable;
// see Send impl above.
unsafe impl Sync for Module {}

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

impl Module {
    /// Open a library, resolve all symbols in `spec`, build CIFs, and return a ready Module.
    pub fn new(
        library_path: &Path,
        spec: ModuleSpec,
        abort_callbacks: AbortCallbacksFn,
        abort_user_data: *const c_void,
    ) -> Result<Arc<Self>> {
        let path_str = library_path
            .to_str()
            .ok_or_else(|| Error::LibraryOpen("path is not valid UTF-8".into()))?;
        let library = LibraryHandle::open(path_str)?;

        // Resolve RustBuffer helper symbols.
        let alloc_ptr = library.lookup_symbol(&spec.rustbuffer_symbols.alloc)?;
        let free_ptr = library.lookup_symbol(&spec.rustbuffer_symbols.free)?;
        let from_bytes_ptr = library.lookup_symbol(&spec.rustbuffer_symbols.from_bytes)?;
        let rb_ops = RustBufferOps {
            alloc_ptr,
            from_bytes_ptr,
            free_ptr,
        };

        // Resolve each function + build its CIF.
        let mut functions = HashMap::with_capacity(spec.functions.len());
        for (name, def) in &spec.functions {
            let symbol = library.lookup_symbol(name)?;
            let mut cif_args: Vec<libffi::middle::Type> = def
                .args
                .iter()
                .map(|t| ffi_type_for(t, &spec.structs))
                .collect::<Result<Vec<_>>>()?;
            if def.has_rust_call_status {
                cif_args.push(libffi::middle::Type::pointer());
            }
            let cif_ret = ffi_type_for(&def.ret, &spec.structs)?;
            let cif = Cif::new(cif_args, cif_ret);
            let arg_layout = ArgLayout::compute(&def.args, def.has_rust_call_status)?;
            functions.insert(
                name.clone(),
                ResolvedFunction {
                    def: def.clone(),
                    symbol,
                    cif,
                    arg_layout,
                },
            );
        }

        // Pre-build CIFs for all callbacks (used by call_callback_ptr).
        let mut callback_cifs = HashMap::with_capacity(spec.callbacks.len());
        for (name, def) in &spec.callbacks {
            let cif_arg_types: Vec<libffi::middle::Type> = def
                .args
                .iter()
                .map(|t| ffi_type_for(t, &spec.structs))
                .collect::<Result<Vec<_>>>()?;
            let cif_ret_type = ffi_type_for(&def.ret, &spec.structs)?;
            callback_cifs.insert(name.clone(), Cif::new(cif_arg_types, cif_ret_type));
        }

        // Pre-compute struct layouts for all structs.
        let mut struct_layouts = HashMap::with_capacity(spec.structs.len());
        for (name, def) in &spec.structs {
            let layout = compute_struct_layout(def, &spec.structs)?;
            struct_layouts.insert(name.clone(), layout);
        }

        Ok(Arc::new(Self {
            library: Mutex::new(Some(library)),
            spec,
            functions,
            callback_cifs,
            struct_layouts,
            rb_ops,
            abort_callbacks,
            abort_user_data,
            lifecycle: crate::lifecycle::UnloadState::new(),
            trampolines: Mutex::new(HashMap::new()),
            trampolines_built: AtomicU64::new(0),
        }))
    }
}

// ---------------------------------------------------------------------------
// Spec accessors (used by frontends at registration time)
// ---------------------------------------------------------------------------

impl Module {
    /// Access the RustBuffer operation function pointers (from_bytes, free).
    pub fn rb_ops(&self) -> &RustBufferOps {
        &self.rb_ops
    }

    /// Look up the definition of a resolved function by name.
    pub fn function_def(&self, fn_name: &str) -> Option<&FunctionDef> {
        self.functions.get(fn_name).map(|r| &r.def)
    }

    /// Access the struct definitions from the module spec.
    pub fn spec_structs(&self) -> &HashMap<String, StructDef> {
        &self.spec.structs
    }

    /// Access the callback definitions from the module spec.
    pub fn spec_callbacks(&self) -> &HashMap<String, CallbackDef> {
        &self.spec.callbacks
    }

    /// Look up the pre-computed C struct layout for the named struct type.
    pub fn struct_field_offsets(&self, struct_name: &str) -> Result<StructLayout> {
        self.struct_layouts
            .get(struct_name)
            .cloned()
            .ok_or_else(|| Error::UnknownStruct(struct_name.to_string()))
    }

    /// Look up a callback definition by name.
    pub(crate) fn callback_def(&self, callback_name: &str) -> Result<&CallbackDef> {
        self.spec
            .callbacks
            .get(callback_name)
            .ok_or_else(|| Error::UnknownCallback(callback_name.to_string()))
    }

    /// Compute the CIF-ordered argument layout for a callback: `[declared_args,
    /// out_return_ptr?, RustCallStatus_ptr?]`. `make_callback_trampoline` builds
    /// its own layout from the same [`callback_arg_layout_for`], so the offsets
    /// a caller packs at are by construction the ones the trampoline unpacks at.
    pub fn callback_arg_layout(&self, callback_name: &str) -> Result<ArgLayout> {
        callback_arg_layout_for(self.callback_def(callback_name)?)
    }

    /// Byte width of the value a callback's trampoline writes back through
    /// libffi's return slot. `make_callback_trampoline` sizes its return buffer
    /// from this, so a bridge that copies those bytes out reads the same width
    /// core wrote.
    pub fn callback_return_size(&self, callback_name: &str) -> Result<usize> {
        crate::callback::return_size(self.callback_def(callback_name)?)
    }
}

/// The layout half of [`Module::callback_arg_layout`], taking an already-resolved
/// definition so a caller holding one does not look it up again.
pub(crate) fn callback_arg_layout_for(def: &CallbackDef) -> Result<ArgLayout> {
    let mut layout_args = def.args.clone();
    if def.out_return {
        layout_args.push(FfiTypeDesc::VoidPointer);
    }
    ArgLayout::compute(&layout_args, def.has_rust_call_status)
}

/// Size and alignment of one argument slot for a player tag name.
///
/// Routes through [`desc_from_name`] and [`slot_size_align`] — the single
/// desc-keyed source of truth — so no size is computed twice in this codebase.
/// `desc_from_name` requires a type name for the `Callback`, `Struct` and
/// `Reference` tags; geometry doesn't depend on it, so a placeholder satisfies
/// it.
///
/// Answers only for the wire vocabulary, so `None` covers four cases: an
/// unknown name; `Struct`, which `slot_size_align` rejects as a bare arg slot;
/// and `VoidPointer`/`MutReference`/`ForeignBytes`, which `desc_from_name` does
/// not build even though `slot_size_align` could size the first two. A bridge
/// sending one of those names must map it to a wire name first.
pub fn slot_size_align_for_name(tag_name: &str) -> Option<(usize, usize)> {
    const PLACEHOLDER_NAME: &str = "_";
    let desc = desc_from_name(tag_name, Some(PLACEHOLDER_NAME)).ok()?;
    slot_size_align(&desc).ok()
}

// Call methods (prepare_call, call, rustbuffer_*, call_callback_ptr) live in call.rs.
// Callback methods (make_callback_trampoline, build_vtable) live in callback.rs.
// Lifecycle methods (is_unloading, unload, unload_force) live in lifecycle.rs.

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Compute the C struct layout for a struct definition using libffi.
fn compute_struct_layout(
    def: &StructDef,
    all_structs: &HashMap<String, StructDef>,
) -> Result<StructLayout> {
    let field_types: Vec<libffi::middle::Type> = def
        .fields
        .iter()
        .map(|f| ffi_type_for(&f.field_type, all_structs))
        .collect::<Result<Vec<_>>>()?;

    let struct_type = libffi::middle::Type::structure(field_types);

    // Force libffi to compute layout by creating a dummy CIF with the
    // struct type as the return type. Cif::new internally calls ffi_prep_cif,
    // which fills in size and alignment on nested struct ffi_types.
    let _cif = Cif::new(vec![], struct_type.clone());

    // Read the layout from the CIF's rtype, which points to the struct
    // ffi_type libffi computed.
    //
    // SAFETY: _cif is alive; as_raw_ptr() returns its inner ffi_cif.
    // rtype points to the struct's ffi_type with populated size/elements.
    let raw = unsafe { (*_cif.as_raw_ptr()).rtype };
    // SAFETY: raw is the rtype pointer populated by ffi_prep_cif above.
    let total_size = unsafe { (*raw).size };
    // SAFETY: raw.elements is a null-terminated array of field ffi_type pointers.
    let raw_elements = unsafe { (*raw).elements };

    let mut fields = Vec::with_capacity(def.fields.len());
    let mut offset = 0usize;
    for i in 0..def.fields.len() {
        // SAFETY: elements is a null-terminated array with at least def.fields.len() entries.
        let elem = unsafe { *raw_elements.add(i) };
        if elem.is_null() {
            break;
        }
        // SAFETY: elem is a valid ffi_type pointer populated by ffi_prep_cif.
        let field_size = unsafe { (*elem).size };
        // SAFETY: same elem pointer; alignment is a small integer.
        let field_align = unsafe { (*elem).alignment as usize };

        // Apply C struct alignment padding.
        offset = (offset + field_align - 1) & !(field_align - 1);
        fields.push(StructFieldLayout {
            offset,
            size: field_size,
        });
        offset += field_size;
    }

    Ok(StructLayout { total_size, fields })
}

/// Precomputed byte layout for a C struct, computed via libffi.
#[derive(Clone)]
pub struct StructLayout {
    pub total_size: usize,
    pub fields: Vec<StructFieldLayout>,
}

#[derive(Clone)]
pub struct StructFieldLayout {
    pub offset: usize,
    pub size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::test_support::{callback_def, test_module};

    #[test]
    fn slot_geometry_matches_the_desc_keyed_source() {
        assert_eq!(slot_size_align_for_name("UInt16"), Some((2, 2)));
        assert_eq!(slot_size_align_for_name("RustBuffer"), Some((24, 8)));
        assert_eq!(slot_size_align_for_name("NotATag"), None);
    }

    /// Covers all four `(out_return, has_rust_call_status)` combinations against
    /// offsets and sizes spelled out here, so a drift this accessor and
    /// `make_callback_trampoline` share still fails. The `out_return` cases are
    /// the ones that matter: they must append a pointer-sized arg_slot *before*
    /// any trailing RustCallStatus slot, matching the CIF's
    /// `[declared_args, out_return_ptr?, RCS_ptr?]`.
    #[test]
    fn callback_arg_layout_matches_trampoline_ordering() {
        let mut callbacks = HashMap::new();
        callbacks.insert(
            "plain".to_string(),
            callback_def(vec![FfiTypeDesc::Int32, FfiTypeDesc::Int64], false, false),
        );
        callbacks.insert(
            "with_rcs".to_string(),
            callback_def(vec![FfiTypeDesc::Int32], true, false),
        );
        callbacks.insert(
            "with_out_return".to_string(),
            callback_def(vec![FfiTypeDesc::Int32], false, true),
        );
        callbacks.insert(
            "with_out_return_and_rcs".to_string(),
            callback_def(vec![FfiTypeDesc::Int32], true, true),
        );
        let m = test_module(callbacks, Default::default());

        // out_return=false, has_rust_call_status=false: just the declared args,
        // packed with natural alignment. No appended slots at all.
        let plain = m.callback_arg_layout("plain").unwrap();
        assert_eq!(plain.arg_slots.len(), 2);
        assert_eq!(plain.arg_slots[0].offset, 0);
        assert_eq!(plain.arg_slots[0].size, 4);
        assert_eq!(plain.arg_slots[1].offset, 8); // Int64 aligned to 8
        assert_eq!(plain.arg_slots[1].size, 8);
        assert_eq!(plain.total_size, 16);
        assert!(plain.rust_call_status_slot.is_none());

        // out_return=false, has_rust_call_status=true: declared args unchanged,
        // RCS is a separate trailing slot, not an arg_slot.
        let with_rcs = m.callback_arg_layout("with_rcs").unwrap();
        assert_eq!(with_rcs.arg_slots.len(), 1);
        assert_eq!(with_rcs.arg_slots[0].offset, 0);
        assert_eq!(with_rcs.arg_slots[0].size, 4);
        let rcs_slot = with_rcs.rust_call_status_slot.as_ref().unwrap();
        assert_eq!(rcs_slot.offset, 8);
        assert_eq!(rcs_slot.size, 8);
        assert_eq!(with_rcs.total_size, 16);

        // out_return=true, has_rust_call_status=false: the out-return pointer is
        // appended as an extra arg_slot (pointer-sized, at offset 8). No RCS slot.
        let with_out = m.callback_arg_layout("with_out_return").unwrap();
        assert_eq!(with_out.arg_slots.len(), 2);
        assert_eq!(with_out.arg_slots[0].offset, 0);
        assert_eq!(with_out.arg_slots[0].size, 4);
        assert_eq!(with_out.arg_slots[1].offset, 8);
        assert_eq!(with_out.arg_slots[1].size, 8);
        assert_eq!(with_out.total_size, 16);
        assert!(with_out.rust_call_status_slot.is_none());

        // out_return=true, has_rust_call_status=true: the risk case. The
        // out-return pointer must land in arg_slots (offset 8, before the RCS
        // slot), and RCS must be the trailing slot at offset 16 — matching CIF
        // order [declared_args, out_return_ptr, RCS_ptr].
        let with_both = m.callback_arg_layout("with_out_return_and_rcs").unwrap();
        assert_eq!(with_both.arg_slots.len(), 2);
        assert_eq!(with_both.arg_slots[0].offset, 0);
        assert_eq!(with_both.arg_slots[0].size, 4);
        assert_eq!(with_both.arg_slots[1].offset, 8);
        assert_eq!(with_both.arg_slots[1].size, 8);
        let rcs_slot = with_both.rust_call_status_slot.as_ref().unwrap();
        assert_eq!(rcs_slot.offset, 16);
        assert_eq!(rcs_slot.size, 8);
        assert_eq!(with_both.total_size, 24);
    }

    #[test]
    fn callback_arg_layout_unknown_name_errors() {
        let m = test_module(HashMap::new(), Default::default());
        assert!(matches!(
            m.callback_arg_layout("nope"),
            Err(Error::UnknownCallback(name)) if name == "nope"
        ));
    }

    #[test]
    fn trampoline_map_returns_what_was_remembered_and_is_keyed_on_both_parts() {
        let m = test_module(Default::default(), Default::default());
        let p = 0x1234 as *const std::ffi::c_void;
        assert!(m.trampoline_for("cb", 7).is_none());
        m.remember_trampoline("cb", 7, p);
        assert_eq!(m.trampoline_for("cb", 7), Some(p));
        assert!(m.trampoline_for("cb", 8).is_none(), "different identity");
        assert!(m.trampoline_for("other", 7).is_none(), "different callback");
    }

    /// Unload empties the map, and it stays empty: a marshal still in flight
    /// when unload ran would otherwise refill it with a pointer into a library
    /// `unload_force` is about to close.
    #[test]
    fn trampoline_map_is_emptied_by_unload_and_stays_empty() {
        let m = test_module(Default::default(), Default::default());
        m.remember_trampoline("cb", 1, 0x1234 as *const std::ffi::c_void);
        m.unload().expect("unload");
        assert!(m.trampoline_for("cb", 1).is_none());

        m.remember_trampoline("cb", 1, 0x1234 as *const std::ffi::c_void);
        assert!(
            m.trampoline_for("cb", 1).is_none(),
            "an insert after unload must not land"
        );
    }
}
