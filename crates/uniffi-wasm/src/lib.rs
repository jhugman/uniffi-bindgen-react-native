/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{cell::RefCell, ptr::NonNull};
pub use wasm_bindgen::prelude::wasm_bindgen as export;
use wasm_bindgen::prelude::*;

pub mod uniffi {
    pub use uniffi::{RustBuffer, RustCallStatus, RustCallStatusCode, UniffiForeignPointerCell};
    pub type VoidPointer = *const std::ffi::c_void;
}

pub trait IntoRust<HighLevel> {
    fn into_rust(v: HighLevel) -> Self;
}
pub trait IntoJs<HighLevel> {
    fn into_js(self) -> HighLevel;
}

macro_rules! identity_into_rust {
    ($high_level:ident, $rust_type:ty) => {
        pub type $high_level = $rust_type;
        impl IntoRust<$high_level> for $rust_type {
            fn into_rust(v: $high_level) -> Self {
                v
            }
        }
        impl IntoJs<$high_level> for $rust_type {
            fn into_js(self) -> $high_level {
                self
            }
        }
    };
}
identity_into_rust!(UInt8, u8);
identity_into_rust!(UInt16, u16);
identity_into_rust!(UInt32, u32);
identity_into_rust!(UInt64, u64);
identity_into_rust!(Int8, i8);
identity_into_rust!(Int16, i16);
identity_into_rust!(Int32, i32);
identity_into_rust!(Int64, i64);
identity_into_rust!(Float32, f32);
identity_into_rust!(Float64, f64);
pub type Handle = u64;

pub type VoidPointer = u64;
impl IntoRust<VoidPointer> for uniffi::VoidPointer {
    fn into_rust(v: VoidPointer) -> Self {
        v as Self
    }
}
impl IntoJs<VoidPointer> for uniffi::VoidPointer {
    fn into_js(self) -> VoidPointer {
        self as VoidPointer
    }
}

pub type ForeignBytes = Vec<u8>;
impl IntoRust<ForeignBytes> for uniffi::RustBuffer {
    fn into_rust(v: ForeignBytes) -> Self {
        Self::from_vec(v)
    }
}
impl IntoJs<ForeignBytes> for uniffi::RustBuffer {
    fn into_js(self) -> ForeignBytes {
        self.destroy_into_vec()
    }
}

macro_rules! uniffi_result {
    ($uniffi_result:ident, $high_level_type:ty) => {
        #[wasm_bindgen]
        extern "C" {
            pub type $uniffi_result;

            #[wasm_bindgen(method, getter)]
            fn code(this: &$uniffi_result) -> i8;

            #[wasm_bindgen(method, getter = errorBuf)]
            fn error_buf(this: &$uniffi_result) -> Option<ForeignBytes>;

            #[wasm_bindgen(method, getter)]
            fn pointee(this: &$uniffi_result) -> Option<$high_level_type>;
        }
        impl $uniffi_result {
            pub fn copy_into_status(&self, rust: &mut uniffi::RustCallStatus) {
                let code = self.code();
                rust.code = uniffi::RustCallStatusCode::try_from(code)
                    .expect("RustCallStatusCode is not valid");
                let buf = uniffi::RustBuffer::from_vec(self.error_buf().unwrap_or_default());
                *rust.error_buf = buf;
            }
        }
    };
}

macro_rules! uniffi_result_with_return {
    ($uniffi_result:ident, $high_level_type:ty, $rust_type:ty) => {
        uniffi_result!($uniffi_result, $high_level_type);
        impl $uniffi_result {
            pub fn copy_into_return(&self, rust: &mut $rust_type) {
                *rust = <$rust_type>::into_rust(self.pointee().unwrap_or_default());
            }
        }
    };
}

// We use Int8, because `()` doesn't cross the FFI, but we don't need to make an `copy_into_return`
uniffi_result!(UniffiResultVoid, Int8);
uniffi_result_with_return!(UniffiResultUInt8, UInt8, u8);
uniffi_result_with_return!(UniffiResultUInt16, UInt16, u16);
uniffi_result_with_return!(UniffiResultUInt32, UInt32, u32);
uniffi_result_with_return!(UniffiResultUInt64, UInt64, u64);
uniffi_result_with_return!(UniffiResultInt8, Int8, i8);
uniffi_result_with_return!(UniffiResultInt16, Int16, i16);
uniffi_result_with_return!(UniffiResultInt32, Int32, i32);
uniffi_result_with_return!(UniffiResultInt64, Int64, i64);
uniffi_result_with_return!(UniffiResultFloat32, Float32, f32);
uniffi_result_with_return!(UniffiResultFloat64, Float64, f64);
uniffi_result_with_return!(UniffiResultForeignBytes, ForeignBytes, uniffi::RustBuffer);

#[wasm_bindgen]
#[derive(Default)]
pub struct RustCallStatus {
    pub code: i8,
    error_buf: Option<ForeignBytes>,
}

#[wasm_bindgen]
impl RustCallStatus {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Default::default()
    }

    #[wasm_bindgen(getter = errorBuf)]
    pub fn error_buf(self) -> Option<ForeignBytes> {
        self.error_buf
    }

    #[wasm_bindgen(setter = errorBuf)]
    pub fn set_error_buf(&mut self, bytes: Option<ForeignBytes>) {
        self.error_buf = bytes;
    }
}

impl RustCallStatus {
    pub fn copy_from(&mut self, rust: uniffi::RustCallStatus) {
        self.code = rust.code as i8;
        let buf = std::mem::ManuallyDrop::into_inner(rust.error_buf).destroy_into_vec();
        self.error_buf = Some(buf);
    }
}

// Needed for UniffiForeignFutureStruct* structs which are sent from JS when
// a foreign callback promise resolves or rejects.
impl IntoRust<RustCallStatus> for ::uniffi::RustCallStatus {
    fn into_rust(js: RustCallStatus) -> Self {
        let code = uniffi::RustCallStatusCode::try_from(js.code)
            .expect("Unexpected error code. This is likely a bug in UBRN");
        let bytes = js.error_buf.unwrap_or_default();
        let error_buf = std::mem::ManuallyDrop::new(uniffi::RustBuffer::from_vec(bytes));
        ::uniffi::RustCallStatus { error_buf, code }
    }
}

impl IntoJs<RustCallStatus> for ::uniffi::RustCallStatus {
    fn into_js(self) -> RustCallStatus {
        let mut status = RustCallStatus::new();
        status.copy_from(self);
        status
    }
}

pub struct ForeignCell<T> {
    inner: RefCell<Option<T>>,
}

impl<T> ForeignCell<T> {
    pub fn new() -> Self {
        Self {
            inner: RefCell::new(None),
        }
    }

    pub fn set(&self, value: T) {
        self.inner.replace(Some(value));
    }

    pub fn with_value<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        let cell = &self.inner;
        let value = cell.borrow();
        let value = value
            .as_ref()
            .expect("ForeignCell accessed before initialization");
        f(value)
    }
}

impl<T> Default for ForeignCell<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Converts a JavaScript vtable reference into a Rust vtable pointer.
///
/// This is used only for long-held references, so much of the safety
/// obligations on the caller will be very easy to meet.
///
/// # Safety
///
/// The caller must ensure:
/// - The returned pointer is properly freed using `Box::from_raw`
/// - The vtable reference remains valid while the pointer is in use
///
/// # Returns
///
/// A non-null pointer to the Rust vtable structure. The pointer must be freed
/// by the caller to prevent memory leaks.
impl<JsType, RsType> IntoRust<JsType> for NonNull<RsType>
where
    RsType: IntoRust<JsType>,
{
    fn into_rust(js: JsType) -> Self {
        let rs = RsType::into_rust(js);
        let ptr = Box::into_raw(Box::new(rs));
        // Safety: Box::into_raw never returns null
        unsafe { NonNull::new_unchecked(ptr) }
    }
}
