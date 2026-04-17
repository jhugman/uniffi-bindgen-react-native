/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::ffi::c_void;
use std::mem::ManuallyDrop;

use dlopen2::raw::Library;
use napi::Result;

/// A thin wrapper around a `dlopen` handle for a UniFFI shared library.
///
/// Created by [`UniffiNativeModule::open`](crate::UniffiNativeModule::open) and
/// stored inside the napi struct for the lifetime of the Node.js module. All
/// symbol lookups go through [`lookup_symbol`](Self::lookup_symbol), which
/// returns raw function pointers that libffi later invokes.
///
/// The library is wrapped in [`ManuallyDrop`] so that `dlclose` is **never**
/// called when the napi struct is garbage-collected. This is necessary because:
///
/// 1. Leaked closures and userdata hold raw function pointers into the library.
/// 2. The library's tokio runtime may still have worker threads executing its
///    code when Node.js GC runs during shutdown.
///
/// On macOS `dlclose` is typically a no-op, but on Linux it actually unmaps
/// the library's code pages — causing SIGSEGV if any thread is still running
/// library code or if a callback trampoline dereferences a function pointer
/// into the unmapped region.
///
/// When issue #379 adds a proper resource registry and coordinated shutdown,
/// this can be replaced with an explicit `close()` that waits for the tokio
/// runtime to drain before calling `dlclose`.
pub struct LibraryHandle {
    pub lib: ManuallyDrop<Library>,
}

// SAFETY: `LibraryHandle` wraps a `dlopen2::raw::Library`, which does not
// implement `Send` itself. However, the underlying `dlopen`/`dlsym` operations
// are thread-safe on POSIX systems (and on Windows). We implement `Send`
// manually so that napi-rs can store this struct in a reference-counted
// JavaScript object. The handle is created on the main Node.js thread and
// stored in an `#[napi]` struct; napi guarantees that the struct's methods
// are only invoked from the main thread's event loop, so no concurrent
// mutation can occur.
unsafe impl Send for LibraryHandle {}

impl LibraryHandle {
    /// Open a shared library at the given filesystem path via `dlopen`.
    pub fn open(path: &str) -> Result<Self> {
        let lib = Library::open(path)
            .map_err(|e| napi::Error::from_reason(format!("dlopen failed for '{path}': {e}")))?;

        Ok(Self {
            lib: ManuallyDrop::new(lib),
        })
    }

    /// Look up a symbol by name in the loaded library, returning a raw pointer.
    ///
    /// The returned pointer is valid for the lifetime of this `LibraryHandle`
    /// (i.e., until the library is closed).
    pub fn lookup_symbol(&self, name: &str) -> Result<*const c_void> {
        // SAFETY: The symbol name comes from JS type definitions validated at
        // registration time — only names that correspond to real UniFFI
        // scaffolding exports are looked up. The returned pointer is to a
        // function (or global) in the loaded shared library and remains valid
        // for the library's lifetime, which is the lifetime of this
        // `LibraryHandle` (the `Library` is dropped only when the napi
        // struct is garbage-collected).
        unsafe {
            self.lib
                .symbol::<*const c_void>(name)
                .map_err(|e| napi::Error::from_reason(format!("Symbol '{name}' not found: {e}")))
        }
    }
}
