/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Shared scaffolding for this crate's unit tests. Several modules load the same
//! `hello-world` fixture cdylib; the build and the path convention live here so
//! a change to either is made once.

use std::collections::HashMap;
use std::ffi::c_void;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};

use crate::module::Module;
use crate::spec::{CallbackDef, ModuleSpec, RustBufferSymbols, StructDef};
use crate::{AbortCallbacksFn, FfiTypeDesc};

/// No-op abort-callbacks hook: these tests never register frontend callbacks.
pub(crate) extern "C" fn noop_abort_callbacks(_user_data: *const c_void) {}

/// Abort-callbacks hook that counts its own invocations into the `AtomicUsize`
/// `user_data` points at, so a test can assert how many times abort actually
/// ran rather than only that some `Result` came back `Ok`.
pub(crate) extern "C" fn counting_abort_callbacks(user_data: *const c_void) {
    // SAFETY: every caller of `test_module_with_counting_abort` passes the
    // `user_data` pointer straight from the `Box::leak`'d `AtomicUsize` it
    // hands back, and that box outlives the module (and this call) because it
    // is leaked, not owned.
    let counter = unsafe { &*(user_data as *const AtomicUsize) };
    counter.fetch_add(1, Ordering::SeqCst);
}

/// Build the `hello-world` fixture cdylib and return the path it lands at.
///
/// Cached: several test modules call this, and cargo serialises concurrent
/// builds of the same target behind a file lock, which shows up as lock-wait
/// noise in the test output.
pub(crate) fn fixture_cdylib_path() -> PathBuf {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let status = Command::new(env!("CARGO"))
            .args(["build", "-p", "uniffi-fixture-hello-world", "--lib"])
            .status()
            .expect("cargo build");
        assert!(status.success(), "fixture build failed");
        let ext = if cfg!(target_os = "macos") {
            "dylib"
        } else if cfg!(target_os = "windows") {
            "dll"
        } else {
            "so"
        };
        let prefix = if cfg!(target_os = "windows") {
            ""
        } else {
            "lib"
        };
        format!(
            "{}/../../target/debug/{prefix}hello_world.{ext}",
            env!("CARGO_MANIFEST_DIR")
        )
        .into()
    })
    .clone()
}

/// The `hello-world` fixture's RustBuffer symbol names.
pub(crate) fn hello_world_rustbuffer_symbols() -> RustBufferSymbols {
    RustBufferSymbols {
        alloc: "ffi_hello_world_rustbuffer_alloc".to_string(),
        free: "ffi_hello_world_rustbuffer_free".to_string(),
        from_bytes: "ffi_hello_world_rustbuffer_from_bytes".to_string(),
    }
}

/// Load the `hello-world` fixture cdylib as a `Module` carrying `callbacks` and
/// `structs`. Enough to exercise layout, trampoline and vtable building; no
/// callback is ever invoked, so the definitions need not match anything real.
pub(crate) fn test_module(
    callbacks: HashMap<String, CallbackDef>,
    structs: HashMap<String, StructDef>,
) -> Arc<Module> {
    let spec = ModuleSpec {
        rustbuffer_symbols: hello_world_rustbuffer_symbols(),
        functions: Default::default(),
        callbacks,
        structs,
    };
    let abort: AbortCallbacksFn = noop_abort_callbacks;
    Module::new(&fixture_cdylib_path(), spec, abort, std::ptr::null()).expect("module load")
}

/// Like `test_module`, but wired to `counting_abort_callbacks` instead of a
/// no-op, so a test can check the hook ran exactly as many times as expected.
/// The counter is leaked, because the hook holds a raw pointer to it and the
/// module outlives no drop of ours. The module is not: the `Arc` dies with the
/// test, leaving only the `dlopen`ed mapping, which every `Module` leaks.
pub(crate) fn test_module_with_counting_abort(
    callbacks: HashMap<String, CallbackDef>,
    structs: HashMap<String, StructDef>,
) -> (Arc<Module>, &'static AtomicUsize) {
    let counter: &'static AtomicUsize = Box::leak(Box::new(AtomicUsize::new(0)));
    let spec = ModuleSpec {
        rustbuffer_symbols: hello_world_rustbuffer_symbols(),
        functions: Default::default(),
        callbacks,
        structs,
    };
    let abort: AbortCallbacksFn = counting_abort_callbacks;
    let user_data = counter as *const AtomicUsize as *const c_void;
    let m = Module::new(&fixture_cdylib_path(), spec, abort, user_data).expect("module load");
    (m, counter)
}

/// A `CallbackDef` with the given argument shape and a `Void` return.
pub(crate) fn callback_def(
    args: Vec<FfiTypeDesc>,
    has_rust_call_status: bool,
    out_return: bool,
) -> CallbackDef {
    CallbackDef {
        args,
        ret: FfiTypeDesc::Void,
        has_rust_call_status,
        out_return,
    }
}
