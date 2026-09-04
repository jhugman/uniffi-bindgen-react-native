/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Trampoline cache: one libffi trampoline per (callback name, JS function).
//!
//! Building a trampoline for a callback argument is expensive and **permanently
//! leaked**, by design: the library may invoke the function pointer from any thread
//! at any later time, so nothing can safely reclaim it. Each one costs
//!
//! - a strong `napi_ref` pinning the JS function,
//! - a leaked `CallbackUserData` (arg layout, cloned type descriptors, `Arc<Module>`),
//! - a fresh JS function plus a fresh `ThreadsafeFunction` (a libuv async handle),
//!   and an entry appended to the env's TSFN registry,
//! - a leaked `TrampolineUserdata`, and
//! - a libffi `Closure`, which owns an executable page.
//!
//! Dispatch used to build one on *every* call that passed a callback, so the cost
//! became a ~3 KB per-call leak. That is reached from ordinary generated code: the
//! async poll loop passes its continuation callback on every poll, so every `await`
//! leaked ~3.2 KB regardless of argument types.
//!
//! Caching removes the per-call part without changing any lifetime: trampolines are
//! still never freed, there is just one per distinct (callback name, function) pair
//! instead of one per call. The async continuation is a module-level `const`, so it
//! is built once and reused for the life of the registration.
//!
//! The trampoline map itself lives in core, keyed on `(callback name, identity)`
//! where `identity` is an opaque `u64`. Minting and remembering that identity is
//! this engine's job, since JS function identity is not something core can observe:
//! a `u64` counter value lives on the JS function object under a hidden per-callback
//! Symbol, mirroring how [`crate::napi_utils::CapacitySymbol`] records RustBuffer
//! ownership on a view. Keying on the function object is what makes reuse safe:
//!
//! - The Symbols belong to one `register()` call, hence one `napi_env`, and a JS
//!   function object cannot cross realms. So an identity is never read back for a
//!   different env than the one that minted it.
//! - The counter backing the identity is per-registration and never reset, so no
//!   two JS functions sharing a callback name can ever be handed the same identity.
//! - A trampoline carries no per-call state. Callbacks receive their handle as an
//!   ordinary argument, so reusing one across calls cannot mix up call state.
//!
//! A caller that passes a freshly created closure on every call gets no benefit —
//! there is nothing stable to key on. It costs slightly more than no cache at all:
//! each call mints an identity and leaves a permanent entry in core's map, on top
//! of the trampoline that was always leaked.
//!
//! Stashing the identity on the JS function (via [`TrampolineCache::set`]) can fail
//! — e.g. a frozen function refuses the property write — and that error is
//! propagated with `?`, failing the whole FFI call. This is asymmetric with the
//! jsi bridge, which catches the equivalent failure and falls through to building
//! (and leaking) an unmemoised trampoline instead of failing the call. Do not
//! assume the two bridges behave the same on this input.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-registration hidden Symbols keyed by callback name, used to stash the
/// identity minted for the JS function that Symbol is read from.
pub struct TrampolineCache {
    /// One `napi_ref`'d Symbol per callback name in the module spec. Symbols are
    /// GC-managed; the strong reference keeps them alive for as long as the JS-side
    /// module facade, which is what makes the cache last as long as the registration.
    symbols: HashMap<String, napi::sys::napi_ref>,
    /// Captured so `Drop` releases each reference against the env that owns it.
    raw_env: napi::sys::napi_env,
    /// Source of fresh identities. Starts at 1 and only ever increases, so within
    /// one registration no two JS functions can be minted the same value.
    next_identity: AtomicU64,
}

// SAFETY: mirrors `CapacitySymbol`. The raw Symbol handles are only ever
// dereferenced on the JS thread that created them; `next_identity` is a plain
// `AtomicU64` and needs no justification of its own. `Send`/`Sync` is required
// because the cache is parked in an `Arc` captured by closures that napi-rs
// requires to be `Send`.
unsafe impl Send for TrampolineCache {}
// SAFETY: see above.
unsafe impl Sync for TrampolineCache {}

impl TrampolineCache {
    /// Create one Symbol per callback name.
    ///
    /// # Safety
    ///
    /// `raw_env` must be a valid `napi_env` for the current callback scope.
    pub unsafe fn new<'a>(
        raw_env: napi::sys::napi_env,
        names: impl Iterator<Item = &'a String>,
    ) -> napi::Result<Self> {
        let mut symbols = HashMap::new();
        for name in names {
            let description = format!("uniffi.trampoline.{name}\0");
            let mut desc_val: napi::sys::napi_value = std::ptr::null_mut();
            // SAFETY: raw_env is valid per this function's contract; description is a
            // local NUL-terminated string and description.len() - 1 excludes the NUL.
            let status = unsafe {
                napi::sys::napi_create_string_utf8(
                    raw_env,
                    description.as_ptr() as *const _,
                    description.len() - 1,
                    &mut desc_val,
                )
            };
            if status != napi::sys::Status::napi_ok {
                return Err(napi::Error::from_reason(
                    "Failed to create trampoline symbol description".to_string(),
                ));
            }

            let mut sym_val: napi::sys::napi_value = std::ptr::null_mut();
            // SAFETY: raw_env is valid per this function's contract; desc_val is
            // the napi_value just created above.
            let status = unsafe { napi::sys::napi_create_symbol(raw_env, desc_val, &mut sym_val) };
            if status != napi::sys::Status::napi_ok {
                return Err(napi::Error::from_reason(
                    "Failed to create trampoline Symbol".to_string(),
                ));
            }

            let mut sym_ref: napi::sys::napi_ref = std::ptr::null_mut();
            // SAFETY: raw_env is valid per this function's contract; sym_val is
            // the napi_value just created above.
            let status =
                unsafe { napi::sys::napi_create_reference(raw_env, sym_val, 1, &mut sym_ref) };
            if status != napi::sys::Status::napi_ok {
                return Err(napi::Error::from_reason(
                    "Failed to reference trampoline Symbol".to_string(),
                ));
            }
            symbols.insert(name.clone(), sym_ref);
        }
        Ok(Self {
            symbols,
            raw_env,
            // Starts at 1, not 0: `get` treats a stashed 0 the same as absent, so 0
            // must never be a value this counter hands out.
            next_identity: AtomicU64::new(1),
        })
    }

    /// Mint the next identity for this registration. Monotonically increasing and
    /// never reset, so every call returns a value no earlier call (for any callback
    /// name) has returned.
    pub fn next_identity(&self) -> u64 {
        self.next_identity.fetch_add(1, Ordering::Relaxed)
    }

    /// Resolve the Symbol for `callback_name`, if the module declares it.
    ///
    /// # Safety
    ///
    /// `raw_env` must match the env this cache was created in.
    unsafe fn symbol(
        &self,
        raw_env: napi::sys::napi_env,
        callback_name: &str,
    ) -> Option<napi::sys::napi_value> {
        let sym_ref = self.symbols.get(callback_name)?;
        let mut sym_val: napi::sys::napi_value = std::ptr::null_mut();
        // SAFETY: raw_env is valid per this function's contract; sym_ref was
        // created by `new` against the same env and not yet deleted.
        let status =
            unsafe { napi::sys::napi_get_reference_value(raw_env, *sym_ref, &mut sym_val) };
        if status != napi::sys::Status::napi_ok || sym_val.is_null() {
            return None;
        }
        Some(sym_val)
    }

    /// Read the identity stashed on `js_fn` for `callback_name`.
    ///
    /// Returns `Ok(None)` when nothing is stashed yet. A lookup failure is also
    /// reported as `Ok(None)` rather than an error: the only cost of a miss is
    /// minting and stashing a fresh identity, which is exactly the pre-cache
    /// behaviour, so a transient napi hiccup must not fail an otherwise valid FFI
    /// call.
    ///
    /// # Safety
    ///
    /// `raw_env` must be valid for the current callback scope; `obj` must be a JS
    /// function value from that scope.
    pub unsafe fn get(
        &self,
        raw_env: napi::sys::napi_env,
        obj: napi::sys::napi_value,
        callback_name: &str,
    ) -> napi::Result<Option<u64>> {
        // SAFETY: raw_env is valid per this function's contract, satisfying `symbol`'s contract.
        let Some(sym_val) = (unsafe { self.symbol(raw_env, callback_name) }) else {
            return Ok(None);
        };

        let mut has = false;
        // SAFETY: raw_env is valid and obj is a JS function per this
        // function's contract; sym_val is the resolved symbol from `symbol` above.
        let status = unsafe { napi::sys::napi_has_own_property(raw_env, obj, sym_val, &mut has) };
        if status != napi::sys::Status::napi_ok || !has {
            return Ok(None);
        }

        let mut val: napi::sys::napi_value = std::ptr::null_mut();
        // SAFETY: raw_env is valid and obj is a JS function per this
        // function's contract; sym_val is the resolved symbol from `symbol` above.
        let status = unsafe { napi::sys::napi_get_property(raw_env, obj, sym_val, &mut val) };
        if status != napi::sys::Status::napi_ok || val.is_null() {
            return Ok(None);
        }

        let mut identity: u64 = 0;
        let mut lossless = false;
        // SAFETY: raw_env is valid per this function's contract; val was just
        // read as this object's own property, so it is live.
        let status = unsafe {
            napi::sys::napi_get_value_bigint_uint64(raw_env, val, &mut identity, &mut lossless)
        };
        if status != napi::sys::Status::napi_ok || !lossless || identity == 0 {
            return Ok(None);
        }
        Ok(Some(identity))
    }

    /// Stash `identity` on `js_fn` for `callback_name`.
    ///
    /// Defined non-enumerable so the marker cannot show up in property enumeration
    /// or structural comparison of a user-supplied function.
    ///
    /// # Safety
    ///
    /// As [`Self::get`].
    pub unsafe fn set(
        &self,
        raw_env: napi::sys::napi_env,
        obj: napi::sys::napi_value,
        callback_name: &str,
        identity: u64,
    ) -> napi::Result<()> {
        // SAFETY: raw_env is valid per this function's contract, satisfying `symbol`'s contract.
        let Some(sym_val) = (unsafe { self.symbol(raw_env, callback_name) }) else {
            return Ok(());
        };

        let mut val: napi::sys::napi_value = std::ptr::null_mut();
        // SAFETY: raw_env is valid per this function's contract; val is a
        // local out-param.
        let status = unsafe { napi::sys::napi_create_bigint_uint64(raw_env, identity, &mut val) };
        if status != napi::sys::Status::napi_ok {
            return Err(napi::Error::from_reason(
                "Failed to create BigInt for trampoline identity".to_string(),
            ));
        }

        let descriptor = napi::sys::napi_property_descriptor {
            utf8name: std::ptr::null(),
            name: sym_val,
            method: None,
            getter: None,
            setter: None,
            value: val,
            attributes: napi::sys::PropertyAttributes::writable,
            data: std::ptr::null_mut(),
        };
        // SAFETY: raw_env is valid and obj is a JS function per this
        // function's contract; descriptor is a local value valid for the call.
        let status = unsafe { napi::sys::napi_define_properties(raw_env, obj, 1, &descriptor) };
        if status != napi::sys::Status::napi_ok {
            return Err(napi::Error::from_reason(
                "Failed to stash trampoline identity".to_string(),
            ));
        }
        Ok(())
    }
}

impl Drop for TrampolineCache {
    fn drop(&mut self) {
        for sym_ref in self.symbols.values() {
            // SAFETY: each ref was created against `self.raw_env` with refcount 1 and
            // has not been deleted. The trampolines the Symbols pointed at stay leaked
            // on purpose — see the module docs.
            unsafe {
                napi::sys::napi_delete_reference(self.raw_env, *sym_ref);
            }
        }
    }
}
