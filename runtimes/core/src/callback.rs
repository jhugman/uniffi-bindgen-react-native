/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Callback trampolines: C fn pointers the loaded library can invoke.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use libffi::low;
use libffi::middle::{Cif, Closure, Type};

use crate::call::ArgLayout;
use crate::cif::ffi_type_for;
use crate::module::Module;
use crate::spec::{CallbackDef, StructDef};
use crate::{Error, FfiTypeDesc, Result};

/// Signature for the function the frontend provides to handle a callback on the JS thread.
pub type OnJsThreadFn = extern "C" fn(args: *const u8, ret: *mut u8, user_data: *const c_void);

/// Signature for the function that dispatches a callback from a non-JS thread to the JS thread.
pub type DispatchFn = extern "C" fn(
    on_js_thread: OnJsThreadFn,
    args: *const u8,
    ret: *mut u8,
    user_data: *const c_void,
);

/// Signature for the function that checks whether the current thread is the JS thread.
pub type IsJsThreadFn = extern "C" fn(user_data: *const c_void) -> bool;

/// An opaque function pointer suitable for storing in a VTable slot.
pub type CallbackFnPtr = *const c_void;

/// Per-trampoline state leaked to a stable address for the lifetime of the module.
pub(crate) struct TrampolineUserdata {
    arg_layout: ArgLayout,
    ret_size: usize,
    on_js_thread: OnJsThreadFn,
    dispatch: DispatchFn,
    is_js_thread: IsJsThreadFn,
    frontend_user_data: *const c_void,
    unloading_flag: Arc<AtomicBool>,
}

// SAFETY: TrampolineUserdata is leaked with a stable address. Its function pointers
// and frontend_user_data are stable for the program lifetime. The unloading_flag Arc
// is Send+Sync.
unsafe impl Send for TrampolineUserdata {}
// SAFETY: All fields are immutable after construction; see Send impl above.
unsafe impl Sync for TrampolineUserdata {}

/// Build a libffi CIF matching the callback's signature.
///
/// CIF arg ordering: [declared_args, out_return_ptr?, RCS_ptr?]
///
/// The out_return pointer (if present) comes before the RustCallStatus pointer.
/// This matches the `ArgLayout` ordering where the out_return slot is an
/// `arg_slot` and the RCS slot is the trailing `rust_call_status_slot`.
pub(crate) fn build_callback_cif(
    def: &CallbackDef,
    structs: &HashMap<String, StructDef>,
) -> Result<Cif> {
    let mut arg_types: Vec<Type> = def
        .args
        .iter()
        .map(|t| ffi_type_for(t, structs))
        .collect::<Result<Vec<_>>>()?;
    let ret_type = if def.out_return {
        // out-return callbacks receive an extra pointer arg and return void.
        // This slot comes before the RCS pointer in the CIF, matching ArgLayout.
        arg_types.push(Type::pointer());
        Type::void()
    } else {
        ffi_type_for(&def.ret, structs)?
    };
    if def.has_rust_call_status {
        arg_types.push(Type::pointer());
    }
    Ok(Cif::new(arg_types, ret_type))
}

/// libffi callback body. Signature matches `libffi::low::Callback<TrampolineUserdata, c_void>`.
///
/// # Safety
///
/// Called by libffi when the closure's code pointer is invoked. The `args` array
/// and `ret` pointer are set up by libffi according to the CIF.
unsafe extern "C" fn trampoline_body(
    _cif: &low::ffi_cif,
    ret: &mut c_void,
    args: *const *const c_void,
    userdata: &TrampolineUserdata,
) {
    use std::sync::atomic::Ordering;

    let ret_ptr = ret as *mut c_void as *mut u8;

    // If the module is shutting down, zero the return value and bail.
    if userdata.unloading_flag.load(Ordering::Acquire) {
        if userdata.ret_size > 0 {
            std::ptr::write_bytes(ret_ptr, 0, userdata.ret_size);
        }
        return;
    }

    // Pack libffi's scattered arg pointers into a contiguous byte buffer.
    // This copies all arg slots (declared args + optional out_return pointer)
    // plus the optional RustCallStatus slot.
    let mut args_buf = vec![0u8; userdata.arg_layout.total_size];
    for (i, slot) in userdata.arg_layout.arg_slots.iter().enumerate() {
        let src = *args.add(i) as *const u8;
        std::ptr::copy_nonoverlapping(src, args_buf.as_mut_ptr().add(slot.offset), slot.size);
    }
    if let Some(ref rcs_slot) = userdata.arg_layout.rust_call_status_slot {
        let rcs_idx = userdata.arg_layout.arg_slots.len();
        let src = *args.add(rcs_idx) as *const u8;
        std::ptr::copy_nonoverlapping(
            src,
            args_buf.as_mut_ptr().add(rcs_slot.offset),
            rcs_slot.size,
        );
    }

    let mut ret_buf = vec![0u8; userdata.ret_size];

    let is_js = (userdata.is_js_thread)(userdata.frontend_user_data);
    if is_js {
        (userdata.on_js_thread)(
            args_buf.as_ptr(),
            ret_buf.as_mut_ptr(),
            userdata.frontend_user_data,
        );
    } else {
        (userdata.dispatch)(
            userdata.on_js_thread,
            args_buf.as_ptr(),
            ret_buf.as_mut_ptr(),
            userdata.frontend_user_data,
        );
    }

    if userdata.ret_size > 0 {
        std::ptr::copy_nonoverlapping(ret_buf.as_ptr(), ret_ptr, userdata.ret_size);
    }
}

/// Compute the byte size of the return value for a callback.
///
/// Delegates to `slot_size_align` for type-to-size mapping, avoiding a
/// duplicate exhaustive match over `FfiTypeDesc`.
fn return_size(def: &CallbackDef) -> Result<usize> {
    if def.out_return {
        return Ok(0);
    }
    match &def.ret {
        FfiTypeDesc::Void => Ok(0),
        other => crate::call::slot_size_align(other).map(|(size, _)| size),
    }
}

impl Module {
    /// Create a libffi closure that acts as a C callback the loaded library can invoke.
    ///
    /// The closure checks the unloading flag, packs args, and dispatches to the JS thread.
    /// Both the closure and its userdata are leaked (stable address, never freed) because
    /// the Rust library may invoke the callback from any thread at any future time.
    pub fn make_callback_trampoline(
        self: &Arc<Self>,
        callback_name: &str,
        on_js_thread: OnJsThreadFn,
        dispatch: DispatchFn,
        is_js_thread: IsJsThreadFn,
        user_data: *const c_void,
    ) -> Result<CallbackFnPtr> {
        let def = self
            .spec
            .callbacks
            .get(callback_name)
            .ok_or_else(|| Error::UnknownCallback(callback_name.to_string()))?;

        // Compute the ArgLayout matching the CIF arg ordering:
        // [declared_args, out_return_ptr?, RCS_ptr?]
        // When out_return is true, the out-return pointer appears as an extra
        // arg_slot (VoidPointer) before the optional rust_call_status_slot.
        let mut layout_args = def.args.clone();
        if def.out_return {
            layout_args.push(crate::FfiTypeDesc::VoidPointer);
        }
        let arg_layout = ArgLayout::compute(&layout_args, def.has_rust_call_status)?;
        let ret_size = return_size(def)?;

        let userdata = Box::new(TrampolineUserdata {
            arg_layout,
            ret_size,
            on_js_thread,
            dispatch,
            is_js_thread,
            frontend_user_data: user_data,
            unloading_flag: self.lifecycle.unloading_flag_arc(),
        });
        let userdata_ref: &'static TrampolineUserdata = Box::leak(userdata);

        // Build the CIF for the closure (Closure takes ownership).
        let cif = build_callback_cif(def, &self.spec.structs)?;
        let closure = Closure::new(cif, trampoline_body, userdata_ref);

        // Extract the code pointer before forgetting the closure.
        // `code_ptr()` returns `&extern "C" fn()`—dereference to get the fn ptr value.
        let fn_ptr = *closure.code_ptr() as *const c_void;
        std::mem::forget(closure);

        Ok(fn_ptr)
    }
}

/// One field in a VTable: the callback name it corresponds to and its function pointer.
pub struct VTableField {
    pub callback_name: String,
    pub fn_ptr: CallbackFnPtr,
}

impl Module {
    /// Build a VTable byte blob from an ordered list of callback function pointers.
    ///
    /// The struct definition in the spec is used to validate field count. Each field
    /// is a pointer-sized slot written in native byte order. The returned pointer is
    /// leaked and valid for the program lifetime.
    pub fn build_vtable(
        self: &Arc<Self>,
        struct_name: &str,
        fields: &[VTableField],
    ) -> Result<*const c_void> {
        let def = self
            .spec
            .structs
            .get(struct_name)
            .ok_or_else(|| Error::UnknownStruct(struct_name.to_string()))?;
        if def.fields.len() != fields.len() {
            return Err(Error::Other(format!(
                "VTable field count mismatch for {struct_name}: spec has {}, got {}",
                def.fields.len(),
                fields.len()
            )));
        }
        let field_size = std::mem::size_of::<*const c_void>();
        let mut bytes: Vec<u8> = Vec::with_capacity(def.fields.len() * field_size);
        for field in fields {
            bytes.extend_from_slice(&(field.fn_ptr as usize).to_ne_bytes());
        }
        let ptr = Box::leak(bytes.into_boxed_slice()).as_ptr();
        Ok(ptr as *const c_void)
    }
}
