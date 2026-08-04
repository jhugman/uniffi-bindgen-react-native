/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Runtime helpers consumed by the Wasm2 UniFFI player.
//!
//! All functions here are exported from the consumer crate's cdylib so that
//! the JS-side player can:
//!   * allocate/free linear-memory scratch space
//!   * forward Rust panics to a JS function the player has installed in
//!     `__indirect_function_table`.
//!
//! Trampoline encoding lives on the TS side, in
//! `runtimes/wasm/core/src/trampoline.ts`. Keeping `wasm-encoder` out of
//! here holds the per-cdylib cost near 700 KB rather than ~5 MB.

use std::alloc::{alloc, dealloc, Layout};
use std::sync::atomic::{AtomicU32, Ordering};

/// # Safety
///
/// `size` and `align` must form a valid `Layout` (non-zero `align`, power of
/// two, and `size` rounded up to `align` must not overflow `isize::MAX`).
/// The caller (the JS-side player) is responsible for later freeing the
/// returned pointer via `__ubrn_free` with the same `size` and `align`.
#[no_mangle]
pub unsafe extern "C" fn __ubrn_alloc(size: u32, align: u32) -> *mut u8 {
    let layout = Layout::from_size_align(size as usize, align as usize)
        .expect("__ubrn_alloc: invalid layout");
    // `std::alloc::alloc` is UB for a zero-sized layout. Zero-sized requests do
    // reach here (e.g. a vtable struct with no fields), so hand back the
    // canonical dangling-but-aligned pointer instead; `__ubrn_free` mirrors it.
    if layout.size() == 0 {
        return layout.align() as *mut u8;
    }
    alloc(layout)
}

/// # Safety
///
/// `ptr` must be a pointer previously returned by `__ubrn_alloc` (or an
/// equivalent allocator call) with the same `size` and `align`, and must
/// not have been freed already. After this call the pointer is invalid.
#[no_mangle]
pub unsafe extern "C" fn __ubrn_free(ptr: *mut u8, size: u32, align: u32) {
    let layout = Layout::from_size_align(size as usize, align as usize)
        .expect("__ubrn_free: invalid layout");
    // Mirrors the zero-sized branch in `__ubrn_alloc`: nothing was allocated.
    if layout.size() == 0 {
        return;
    }
    dealloc(ptr, layout);
}

/// Index in `__indirect_function_table` of a player-installed JS function
/// with signature `fn(msg_ptr: *const u8, msg_len: u32)`. The panic hook
/// loads this slot and dispatches to it on every Rust panic. `0` means
/// "no logger installed yet" — panics are silent rather than aborting.
///
/// A table index rather than an `extern "C"` import, so linking cdylibs need
/// not declare `env.__ubrn_panic_log`. Node's `--experimental-wasm-modules`,
/// which the legacy Wasm flavor loads through, cannot resolve an unrecognised
/// `env` module.
static PANIC_LOG_FN: AtomicU32 = AtomicU32::new(0);

/// Install (or replace) the JS function the panic hook dispatches to.
/// `table_index` must point at a function with signature
/// `(i32 msg_ptr, i32 msg_len) -> ()` in the cdylib's
/// `__indirect_function_table`.
#[no_mangle]
pub extern "C" fn __ubrn_set_panic_log(table_index: u32) {
    PANIC_LOG_FN.store(table_index, Ordering::Relaxed);
}

/// Install the Rust panic hook. Idempotent — safe to call multiple times.
/// The player invokes this once during `open()`. Panics fired before
/// `__ubrn_set_panic_log` is called land at slot 0 and are dropped
/// silently.
#[no_mangle]
pub extern "C" fn __ubrn_install_panic_hook() {
    use std::sync::Once;
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let idx = PANIC_LOG_FN.load(Ordering::Relaxed);
            if idx == 0 {
                return;
            }
            // On wasm32, a Rust `fn` pointer is exactly the
            // `__indirect_function_table` index — same 32-bit
            // representation — so transmute round-trips cleanly.
            type PanicLogFn = extern "C" fn(*const u8, u32);
            let f: PanicLogFn = unsafe { std::mem::transmute(idx as usize) };
            let msg = format!("{info}");
            f(msg.as_ptr(), msg.len() as u32);
        }));
    });
}
