//! C-layout structs matching UniFFI's ABI conventions.
//!
//! These `#[repr(C)]` types mirror the structs that UniFFI-generated Rust code
//! exports across the FFI boundary. They must match the exact field order, sizes,
//! and alignment that the target library expects, because we pass them by value
//! (for `RustBufferC`) or by pointer (for `RustCallStatusC`) through libffi.
//!
//! There are three struct types and two function-pointer type aliases:
//!
//! - [`RustBufferC`]: The **owned** byte buffer — `{ capacity: u64, len: u64, data: *mut u8 }`.
//!   UniFFI uses this for passing serialized compound types across the boundary.
//! - [`ForeignBytesC`]: A **borrowed** byte view — `{ len: i32, data: *const u8 }`.
//!   Used only in `rustbuffer_from_bytes` to hand owned-by-JS bytes to Rust without copying.
//! - [`RustCallStatusC`]: The error-reporting out-parameter — `{ code: i8, error_buf: RustBuffer }`.
//!   The `error_buf` fields are **inlined** (not nested) because we need direct field access
//!   when constructing and inspecting the struct from Rust, and because `#[repr(C)]` layout
//!   of a struct with inlined fields is identical to one with a nested sub-struct.
//! - [`RustBufferFromBytesFn`] and [`RustBufferFreeFn`]: Function-pointer types for the
//!   two `rustbuffer` management symbols (`*_rustbuffer_from_bytes` and `*_rustbuffer_free`)
//!   that are resolved from the loaded library at registration time.

/// C layout of UniFFI's `RustBuffer`: an **owned** byte buffer passed by value across the FFI.
///
/// Fields: `capacity` (allocated size), `len` (used size), `data` (heap pointer).
/// The Rust side is responsible for allocation and deallocation; JS must call
/// the library's `rustbuffer_free` to release buffers it receives.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct RustBufferC {
    pub capacity: u64,
    pub len: u64,
    pub data: *mut u8,
}

/// C layout of UniFFI's `ForeignBytes`: a **borrowed** byte view into JS-owned memory.
///
/// Used exclusively by the `rustbuffer_from_bytes` FFI function, which copies the
/// contents into a new `RustBuffer`. The JS caller must keep the backing `Buffer`
/// alive until the call returns.
#[repr(C)]
pub(crate) struct ForeignBytesC {
    pub len: i32,
    pub data: *const u8,
}

/// C layout of UniFFI's `RustCallStatus`: the out-parameter every scaffolding
/// function writes its error status into.
///
/// Logically `{ code: i8, error_buf: RustBuffer }`, but the `RustBuffer` fields
/// are inlined here (`error_buf_capacity`, `error_buf_len`, `error_buf_data`)
/// so we can read and write them individually without constructing a nested struct.
/// The `#[repr(C)]` layout is byte-identical either way.
#[repr(C)]
pub(crate) struct RustCallStatusC {
    pub code: i8,
    // RustBuffer fields inlined: capacity, len, data — see module docs.
    pub error_buf_capacity: u64,
    pub error_buf_len: u64,
    pub error_buf_data: *mut u8,
}

impl Default for RustCallStatusC {
    /// A zeroed status: code 0 (success) with a null, empty error buffer.
    fn default() -> Self {
        Self {
            code: 0,
            error_buf_capacity: 0,
            error_buf_len: 0,
            error_buf_data: std::ptr::null_mut(),
        }
    }
}

/// Signature of the `*_rustbuffer_from_bytes` symbol: takes borrowed bytes and an
/// out-parameter for call status, returns an owned `RustBufferC`.
pub(crate) type RustBufferFromBytesFn =
    unsafe extern "C" fn(ForeignBytesC, *mut RustCallStatusC) -> RustBufferC;

/// Signature of the `*_rustbuffer_free` symbol: takes an owned `RustBufferC` and
/// an out-parameter for call status, deallocates the buffer.
pub(crate) type RustBufferFreeFn = unsafe extern "C" fn(RustBufferC, *mut RustCallStatusC);

/// Resolved function pointers for RustBuffer lifecycle management.
///
/// These two symbols are resolved once during [`crate::register::register`] and
/// threaded through to every site that needs to allocate or free RustBuffers.
#[derive(Clone, Copy)]
pub(crate) struct RustBufferOps {
    pub from_bytes_ptr: *const std::ffi::c_void,
    pub free_ptr: *const std::ffi::c_void,
}
