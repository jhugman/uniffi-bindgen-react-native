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
use std::sync::{Arc, Mutex};

use libffi::low::CodePtr;
use libffi::middle::Cif;

use crate::call::{ArgLayout, CallReturn};
use crate::cif::ffi_type_for;
use crate::ffi_c_types::RustBufferOps;
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
    /// Build libffi `Arg` references from `arg_bytes` and call the resolved symbol.
    ///
    /// The `Arg` pointers into `arg_bytes` are stack-local — they are created,
    /// passed to `cif.call`, and dropped within this method.
    pub(crate) fn invoke(&self, arg_bytes: &[u8]) -> Result<CallReturn> {
        let buf_ptr = arg_bytes.as_ptr();

        let mut ffi_args: Vec<libffi::middle::Arg> =
            Vec::with_capacity(self.arg_layout.arg_slots.len() + 1);
        for slot in &self.arg_layout.arg_slots {
            // SAFETY: slot.offset + slot.size <= arg_bytes.len(), enforced by ArgLayout::compute.
            let slot_ptr = unsafe { buf_ptr.add(slot.offset) };
            // SAFETY: slot_ptr points into arg_bytes which is alive for this call.
            ffi_args.push(unsafe { libffi::middle::arg(&*slot_ptr) });
        }
        if let Some(rcs_slot) = &self.arg_layout.rust_call_status_slot {
            // SAFETY: rcs_slot.offset is within arg_bytes, same reasoning as above.
            let slot_ptr = unsafe { buf_ptr.add(rcs_slot.offset) };
            // SAFETY: slot_ptr points into arg_bytes, same reasoning as above.
            ffi_args.push(unsafe { libffi::middle::arg(&*slot_ptr) });
        }

        let code_ptr = CodePtr::from_ptr(self.symbol);
        let ffi_args = &ffi_args;

        // SAFETY: The CIF was built from the same FunctionDef as the arg buffer layout.
        // Each ffi_arg points into arg_bytes (alive for this call). code_ptr is a
        // resolved symbol from a loaded library alive for the Module's lifetime.
        let ret = unsafe {
            match &self.def.ret {
                FfiTypeDesc::Void => {
                    self.cif.call::<()>(code_ptr, ffi_args);
                    CallReturn::Void
                }
                FfiTypeDesc::UInt8 => CallReturn::U8(self.cif.call(code_ptr, ffi_args)),
                FfiTypeDesc::Int8 => CallReturn::I8(self.cif.call(code_ptr, ffi_args)),
                FfiTypeDesc::UInt16 => CallReturn::U16(self.cif.call(code_ptr, ffi_args)),
                FfiTypeDesc::Int16 => CallReturn::I16(self.cif.call(code_ptr, ffi_args)),
                FfiTypeDesc::UInt32 => CallReturn::U32(self.cif.call(code_ptr, ffi_args)),
                FfiTypeDesc::Int32 => CallReturn::I32(self.cif.call(code_ptr, ffi_args)),
                FfiTypeDesc::UInt64 | FfiTypeDesc::Handle => {
                    CallReturn::U64(self.cif.call(code_ptr, ffi_args))
                }
                FfiTypeDesc::Int64 => CallReturn::I64(self.cif.call(code_ptr, ffi_args)),
                FfiTypeDesc::Float32 => CallReturn::F32(self.cif.call(code_ptr, ffi_args)),
                FfiTypeDesc::Float64 => CallReturn::F64(self.cif.call(code_ptr, ffi_args)),
                FfiTypeDesc::RustBuffer => {
                    CallReturn::RustBuffer(self.cif.call(code_ptr, ffi_args))
                }
                FfiTypeDesc::VoidPointer
                | FfiTypeDesc::Reference(_)
                | FfiTypeDesc::MutReference(_)
                | FfiTypeDesc::Callback(_) => {
                    let v: usize = self.cif.call(code_ptr, ffi_args);
                    CallReturn::Pointer(v)
                }
                other => {
                    return Err(Error::UnsupportedType(format!(
                        "return type {other:?} not yet supported"
                    )));
                }
            }
        };

        Ok(ret)
    }
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
}

// SAFETY: all interior mutability is via atomics in UnloadState; ResolvedFunction is immutable
// after construction. Raw pointers (abort_user_data, rb_ops) are stable for the Module lifetime.
unsafe impl Send for Module {}
// SAFETY: Mutex guards library; all other fields are immutable; see Send impl above.
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
        let _alloc = library.lookup_symbol(&spec.rustbuffer_symbols.alloc)?;
        let free_ptr = library.lookup_symbol(&spec.rustbuffer_symbols.free)?;
        let from_bytes_ptr = library.lookup_symbol(&spec.rustbuffer_symbols.from_bytes)?;
        let rb_ops = RustBufferOps {
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
