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
