/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Lifecycle state for orderly module shutdown.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

pub(crate) struct UnloadState {
    unloading: Arc<AtomicBool>,
    in_flight: AtomicU64,
}

impl UnloadState {
    pub fn new() -> Self {
        Self {
            unloading: Arc::new(AtomicBool::new(false)),
            in_flight: AtomicU64::new(0),
        }
    }

    pub fn is_unloading(&self) -> bool {
        self.unloading.load(Ordering::Acquire)
    }

    /// Returns a clone of the Arc wrapping the unloading flag, so callback
    /// trampolines can cheaply check it without holding a reference to the
    /// full `UnloadState`.
    pub(crate) fn unloading_flag_arc(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.unloading)
    }

    pub(crate) fn try_begin_call(&self) -> bool {
        if self.is_unloading() {
            return false;
        }
        self.in_flight.fetch_add(1, Ordering::AcqRel);
        if self.is_unloading() {
            self.in_flight.fetch_sub(1, Ordering::AcqRel);
            return false;
        }
        true
    }

    pub(crate) fn end_call(&self) {
        self.in_flight.fetch_sub(1, Ordering::AcqRel);
    }

    pub(crate) fn begin_unload(&self) -> bool {
        self.unloading
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn wait_for_drain(&self) {
        while self.in_flight.load(Ordering::Acquire) > 0 {
            std::thread::yield_now();
        }
    }
}

use crate::module::Module;
use crate::Result;

impl Module {
    /// Returns true if this module is shutting down or already shut down.
    pub fn is_unloading(&self) -> bool {
        self.lifecycle.is_unloading()
    }

    /// Stop this module serving its frontend: set the unloading flag, then call
    /// whatever abort hook was registered. Nothing is drained, nothing is freed
    /// and the library stays mapped, so there is nothing to wait for.
    ///
    /// Releasing parked callbacks is the abort hook's job, and the hook is the
    /// frontend's to supply.
    ///
    /// The trampoline map is left intact on purpose: the flag alone makes every
    /// entry inert, and clearing it would only let a later marshal build a
    /// replacement nothing can ever call.
    ///
    /// One way, and it forecloses the others: `unload` keys off the same flag,
    /// so after a `disarm` it early-returns without draining or clearing the
    /// map, and an `unload_force` behind it would `dlclose` a library with
    /// in-flight calls and a map still full of pointers into it. No frontend
    /// does both today — jsi disarms and never unloads, napi unloads and never
    /// disarms — and one that wanted both would have to give `unload` a
    /// stronger test than the flag.
    ///
    /// Returns `Ok(())` immediately if the module is already unloading.
    pub fn disarm(&self) -> Result<()> {
        if !self.lifecycle.begin_unload() {
            return Ok(());
        }
        (self.abort_callbacks)(self.abort_user_data);
        Ok(())
    }

    /// Initiate orderly shutdown: set the unloading flag, abort frontend callbacks,
    /// then wait for in-flight calls to drain.
    ///
    /// Returns `Ok(())` immediately if the module is already unloading or unloaded.
    pub fn unload(&self) -> Result<()> {
        if !self.lifecycle.begin_unload() {
            return Ok(()); // already unloading/unloaded
        }
        (self.abort_callbacks)(self.abort_user_data);
        self.lifecycle.wait_for_drain();
        self.trampolines
            .lock()
            .expect("trampolines mutex poisoned")
            .clear();
        Ok(())
    }

    /// Perform orderly shutdown and then close (dlclose) the underlying library.
    ///
    /// # Safety contract
    ///
    /// The caller must ensure no code from the library is still executing after
    /// this call returns. In practice, `unload()` drains in-flight calls first.
    pub fn unload_force(&self) -> Result<()> {
        self.unload()?;
        let lib = self.library.lock().expect("library mutex poisoned").take();
        if let Some(lib) = lib {
            // SAFETY: unload() has drained all in-flight calls and set the
            // unloading flag, so no code from the library should be executing.
            unsafe {
                lib.close();
            }
        }
        Ok(())
    }
}
