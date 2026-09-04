/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Callback trampolines: C fn pointers the loaded library can invoke.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, PoisonError};

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
            // SAFETY: libffi sized the storage `ret` points at from the same CIF
            // `ret_size` was computed from, so it is valid for that many writable
            // bytes.
            unsafe { std::ptr::write_bytes(ret_ptr, 0, userdata.ret_size) };
        }
        return;
    }

    // Pack libffi's scattered arg pointers into a contiguous byte buffer.
    // This copies all arg slots (declared args + optional out_return pointer)
    // plus the optional RustCallStatus slot.
    let mut args_buf = vec![0u8; userdata.arg_layout.total_size];
    for (i, slot) in userdata.arg_layout.arg_slots.iter().enumerate() {
        // SAFETY: the CIF carries one argument per `arg_slots` entry, so libffi's
        // `args` array holds at least that many pointers and the `i`th addresses
        // `slot.size` readable bytes.
        let src = unsafe { *args.add(i) } as *const u8;
        let dst = args_buf.as_mut_ptr();
        // SAFETY: `ArgLayout::compute` keeps `slot.offset + slot.size` within
        // `total_size`, which is `args_buf`'s length. `args_buf` is a fresh local
        // allocation, so it cannot overlap libffi's argument storage.
        unsafe { std::ptr::copy_nonoverlapping(src, dst.add(slot.offset), slot.size) };
    }
    if let Some(ref rcs_slot) = userdata.arg_layout.rust_call_status_slot {
        let rcs_idx = userdata.arg_layout.arg_slots.len();
        // SAFETY: a layout with a RustCallStatus slot was built into a CIF with a
        // trailing pointer argument, so `args` holds an entry at `rcs_idx`
        // addressing `rcs_slot.size` readable bytes.
        let src = unsafe { *args.add(rcs_idx) } as *const u8;
        let dst = args_buf.as_mut_ptr();
        // SAFETY: as in the argument loop above — the slot lies within
        // `args_buf`, which cannot overlap libffi's argument storage.
        unsafe { std::ptr::copy_nonoverlapping(src, dst.add(rcs_slot.offset), rcs_slot.size) };
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
        // SAFETY: `ret_buf` is `ret_size` bytes long and libffi sized the storage
        // `ret` points at from the same CIF, so both are valid for that many
        // bytes; `ret_buf` is a fresh local allocation and cannot overlap it.
        unsafe { std::ptr::copy_nonoverlapping(ret_buf.as_ptr(), ret_ptr, userdata.ret_size) };
    }
}

/// Compute the byte size of the return value for a callback.
///
/// Delegates to `slot_size_align` for type-to-size mapping, avoiding a
/// duplicate exhaustive match over `FfiTypeDesc`.
pub(crate) fn return_size(def: &CallbackDef) -> Result<usize> {
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
        // A trampoline built now can never be called: `trampoline_body` reads
        // the same flag and returns. Both the closure and its userdata are
        // leaked by design, so building one is unrecoverable loss.
        if self.lifecycle.is_unloading() {
            return Err(Error::Unloading);
        }

        // The trampoline unpacks at exactly the offsets `callback_arg_layout`
        // publishes, so it shares that computation rather than recomputing.
        let def = self.callback_def(callback_name)?;
        let arg_layout = crate::module::callback_arg_layout_for(def)?;
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

impl Module {
    /// Look up a previously-remembered trampoline for this callback name and JS
    /// function identity, if any.
    ///
    /// Reached from `extern "C"` bridge exports that report failure by returning
    /// null, so a poisoned lock yields the stored map rather than unwinding
    /// across that boundary. Nothing under this lock can leave the map in a
    /// state a reader would misread: entries are plain addresses.
    pub fn trampoline_for(&self, callback_name: &str, identity: u64) -> Option<*const c_void> {
        let trampolines = self
            .trampolines
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        trampolines
            .get(&identity)?
            .get(callback_name)
            .map(|addr| *addr as *const c_void)
    }

    /// Remember a trampoline for this callback name and JS function identity, so
    /// a later call with the same pair can reuse it instead of building a new one.
    ///
    /// Correctness rests entirely on the caller: within one module, `identity`
    /// must name one JS function and only that JS function. Two distinct JS
    /// functions sharing an identity silently dispatch a live call into the
    /// wrong callback — it returns a wrong result rather than crashing. Core
    /// stores and reuses whatever pair it is given; it cannot observe JS
    /// identity itself, so it cannot check this.
    ///
    /// Dropped once the module is unloading, which two shutdown paths lean on.
    /// `unload` empties the map after draining, so an insert landing behind it
    /// would hand a later lookup a pointer into a library `unload_force` has
    /// since closed. `disarm` keeps the map instead of clearing it, and that is
    /// only sound because this guard means nothing is added after the flag is
    /// set: what the map holds is a fixed set of entries the flag has already
    /// made inert.
    pub fn remember_trampoline(&self, callback_name: &str, identity: u64, fn_ptr: *const c_void) {
        if self.lifecycle.is_unloading() {
            return;
        }
        let mut trampolines = self
            .trampolines
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        trampolines
            .entry(identity)
            .or_default()
            .insert(callback_name.to_string(), fn_ptr as usize);
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
        // Every fn pointer this blob would carry is already inert, and the blob
        // itself is leaked — so build nothing.
        if self.lifecycle.is_unloading() {
            return Err(Error::Unloading);
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    use std::sync::atomic::Ordering;

    use crate::spec::StructField;
    use crate::test_support::{callback_def, test_module, test_module_with_counting_abort};

    fn module_with_one_callback() -> Arc<Module> {
        let mut callbacks = HashMap::new();
        callbacks.insert(
            "method".to_string(),
            callback_def(vec![FfiTypeDesc::Int32], true, false),
        );
        test_module(callbacks, Default::default())
    }

    /// Disarm is the whole of what a reload needs: the flag set, nothing freed,
    /// nothing forgotten. The map must survive — an entry read after the flag is
    /// set is inert, while a cleared map lets the next marshal build a
    /// replacement that can never be called.
    #[test]
    fn disarm_sets_the_flag_and_keeps_the_trampoline_map() {
        let m = module_with_one_callback();
        let remembered = 0xabcd_usize as *const c_void;
        m.remember_trampoline("method", 7, remembered);
        assert!(!m.is_unloading());

        m.disarm().expect("disarm");

        assert!(m.is_unloading());
        assert_eq!(
            m.trampoline_for("method", 7),
            Some(remembered),
            "disarm cleared the reuse map, so a later marshal would build again",
        );
    }

    /// A second runtime teardown, or a teardown racing an `unload`, must not
    /// re-run the abort hook.
    #[test]
    fn disarm_is_idempotent() {
        let (m, abort_calls) = test_module_with_counting_abort(HashMap::new(), Default::default());
        m.disarm().expect("first disarm");
        m.disarm().expect("second disarm");
        assert!(m.is_unloading());
        assert_eq!(
            abort_calls.load(Ordering::SeqCst),
            1,
            "second disarm must not re-run the abort hook",
        );
    }

    extern "C" fn on_js(_args: *const u8, _ret: *mut u8, _user_data: *const c_void) {}

    extern "C" fn dispatch(
        _on_js_thread: OnJsThreadFn,
        _args: *const u8,
        _ret: *mut u8,
        _user_data: *const c_void,
    ) {
    }

    extern "C" fn is_js(_user_data: *const c_void) -> bool {
        true
    }

    /// A trampoline built after disarm is dead on arrival — `trampoline_body`
    /// reads the same flag and returns — so building one only leaks a libffi
    /// closure and its userdata, both of which are leaked by design and can
    /// never be reclaimed.
    #[test]
    fn make_callback_trampoline_refuses_after_disarm() {
        let m = module_with_one_callback();
        m.make_callback_trampoline("method", on_js, dispatch, is_js, std::ptr::null())
            .expect("an armed module builds a trampoline");

        m.disarm().expect("disarm");

        let err = m
            .make_callback_trampoline("method", on_js, dispatch, is_js, std::ptr::null())
            .expect_err("a disarmed module must refuse to build");
        assert!(matches!(err, Error::Unloading), "got {err:?}");
    }

    /// Same reasoning one level up: every fn pointer a vtable would carry is
    /// inert after disarm, so the leaked blob would only ever be read by a
    /// caller that cannot use it.
    #[test]
    fn build_vtable_refuses_after_disarm() {
        let mut callbacks = HashMap::new();
        callbacks.insert(
            "method".to_string(),
            callback_def(vec![FfiTypeDesc::Int32], true, false),
        );
        let mut structs = HashMap::new();
        structs.insert(
            "TestVTable".to_string(),
            StructDef {
                fields: vec![StructField {
                    name: "method".to_string(),
                    field_type: FfiTypeDesc::Callback("method".to_string()),
                }],
            },
        );
        let m = test_module(callbacks, structs);
        let fields = [VTableField {
            callback_name: "method".to_string(),
            fn_ptr: std::ptr::null(),
        }];

        m.build_vtable("TestVTable", &fields)
            .expect("an armed module builds a vtable");

        m.disarm().expect("disarm");

        let err = m
            .build_vtable("TestVTable", &fields)
            .expect_err("a disarmed module must refuse to build");
        assert!(matches!(err, Error::Unloading), "got {err:?}");
    }
}
