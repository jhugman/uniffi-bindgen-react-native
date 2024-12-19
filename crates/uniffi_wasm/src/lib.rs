/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
pub use wasm_bindgen::prelude::wasm_bindgen as export;
use wasm_bindgen::prelude::*;

pub mod uniffi {
    pub use uniffi::{RustBuffer, RustCallStatus};
    pub type VoidPointer = *const std::ffi::c_void;
}

pub trait IntoRust<HighLevel> {
    fn into_rust(v: HighLevel) -> Self;
    fn into_js(self) -> HighLevel;
}

macro_rules! identity_into_rust {
    ($high_level:ident, $rust_type:ty) => {
        pub type $high_level = $rust_type;
        impl IntoRust<$high_level> for $rust_type {
            fn into_rust(v: $high_level) -> Self {
                v
            }
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
    fn into_js(self) -> VoidPointer {
        self as VoidPointer
    }
}

pub type ForeignBytes = Vec<u8>;
impl IntoRust<ForeignBytes> for uniffi::RustBuffer {
    fn into_rust(v: ForeignBytes) -> Self {
        Self::from_vec(v)
    }
    fn into_js(self) -> ForeignBytes {
        self.destroy_into_vec()
    }
}

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
}

impl RustCallStatus {
    pub fn copy_into(&mut self, rust: uniffi::RustCallStatus) {
        self.code = rust.code as i8;
        let buf = std::mem::ManuallyDrop::into_inner(rust.error_buf).destroy_into_vec();
        self.error_buf = Some(buf);
    }
}
