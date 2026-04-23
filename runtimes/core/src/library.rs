/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Thin wrapper around dlopen2's `Library` + symbol lookup.

use std::ffi::c_void;
use std::mem::ManuallyDrop;

use dlopen2::raw::Library;

use crate::{Error, Result};

pub struct LibraryHandle {
    pub(crate) lib: ManuallyDrop<Library>,
}

// SAFETY: dlopen/dlsym are thread-safe on POSIX and Windows.
unsafe impl Send for LibraryHandle {}
// SAFETY: The Library handle is read-only after construction; see Send impl above.
unsafe impl Sync for LibraryHandle {}

impl LibraryHandle {
    pub fn open(path: &str) -> Result<Self> {
        let lib = Library::open(path).map_err(|e| Error::LibraryOpen(format!("{path}: {e}")))?;
        Ok(Self {
            lib: ManuallyDrop::new(lib),
        })
    }

    pub fn lookup_symbol(&self, name: &str) -> Result<*const c_void> {
        // SAFETY: symbol name came from a ModuleSpec; returned pointer is valid
        // for the lifetime of the LibraryHandle.
        unsafe {
            self.lib
                .symbol::<*const c_void>(name)
                .map_err(|e| Error::SymbolNotFound(format!("{name}: {e}")))
        }
    }

    /// Unmap the library via dlclose. The caller is responsible for ensuring
    /// no code pages from the library are still being executed.
    pub(crate) unsafe fn close(mut self) {
        // SAFETY: caller asserts no code from `self.lib` is executing.
        unsafe { ManuallyDrop::drop(&mut self.lib) };
    }
}
