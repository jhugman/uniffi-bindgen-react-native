/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Alignment-agnostic byte<->scalar helpers for argument slots, struct fields,
//! and any other `&[u8]` / `&mut [u8]` carrying a single C-shaped value.
//!
//! [`PreparedCall`](crate::PreparedCall) hands the bridge layer raw byte slices
//! whose offsets are computed from per-type alignment ([`ArgLayout`](crate::ArgLayout)),
//! but the underlying buffer is `Vec<u8>` whose base only guarantees u8 alignment.
//! These helpers compile to a single store/load in release mode and avoid the
//! `to_ne_bytes` / `from_ne_bytes(_.try_into().unwrap())` boilerplate from
//! sprawling across bridge layers.
//!
//! Each `write_*` writes the leading bytes of `slot`; each `read_*` reads the
//! leading bytes. Slots are expected to be sized correctly by the caller (via
//! [`slot_size_align`](crate::slot_size_align)); functions panic on undersized
//! slots, which would indicate a layout bug.

use std::ffi::c_void;

use crate::ffi_c_types::RustBufferC;

macro_rules! scalar_slot {
    ($write:ident, $read:ident, $t:ty) => {
        #[doc = concat!("Write a `", stringify!($t), "` into the leading bytes of `slot`.")]
        #[inline]
        pub fn $write(slot: &mut [u8], v: $t) {
            slot[..std::mem::size_of::<$t>()].copy_from_slice(&v.to_ne_bytes());
        }

        #[doc = concat!("Read a `", stringify!($t), "` from the leading bytes of `slot`.")]
        #[inline]
        pub fn $read(slot: &[u8]) -> $t {
            <$t>::from_ne_bytes(slot[..std::mem::size_of::<$t>()].try_into().unwrap())
        }
    };
}

scalar_slot!(write_u8, read_u8, u8);
scalar_slot!(write_i8, read_i8, i8);
scalar_slot!(write_u16, read_u16, u16);
scalar_slot!(write_i16, read_i16, i16);
scalar_slot!(write_u32, read_u32, u32);
scalar_slot!(write_i32, read_i32, i32);
scalar_slot!(write_u64, read_u64, u64);
scalar_slot!(write_i64, read_i64, i64);
scalar_slot!(write_f32, read_f32, f32);
scalar_slot!(write_f64, read_f64, f64);

/// Write a pointer-sized value into the leading bytes of `slot`.
#[inline]
pub fn write_pointer(slot: &mut [u8], ptr: *const c_void) {
    let bytes = (ptr as usize).to_ne_bytes();
    slot[..bytes.len()].copy_from_slice(&bytes);
}

/// Read a pointer-sized value from the leading bytes of `slot` as `usize`.
///
/// Callers cast to the desired pointer type (`*const c_void`, `*mut T`, ...).
#[inline]
pub fn read_pointer(slot: &[u8]) -> usize {
    let n = std::mem::size_of::<usize>();
    usize::from_ne_bytes(slot[..n].try_into().unwrap())
}

/// Convert a [`RustBufferC`] to its raw `repr(C)` byte representation.
#[inline]
pub fn rust_buffer_to_bytes(rb: &RustBufferC) -> [u8; std::mem::size_of::<RustBufferC>()] {
    // SAFETY: RustBufferC is repr(C) and all its fields' bit patterns are valid for u8.
    unsafe { std::mem::transmute(*rb) }
}

/// Write a [`RustBufferC`] into the leading bytes of `slot` as its raw `repr(C)` form.
#[inline]
pub fn write_rust_buffer(slot: &mut [u8], rb: RustBufferC) {
    let bytes = rust_buffer_to_bytes(&rb);
    slot[..bytes.len()].copy_from_slice(&bytes);
}

/// Read a [`RustBufferC`] from the leading bytes of `slot`.
///
/// The slot must be at least `size_of::<RustBufferC>()` bytes; the read is
/// unaligned-safe because the slot is byte-aligned.
#[inline]
pub fn read_rust_buffer(slot: &[u8]) -> RustBufferC {
    debug_assert!(slot.len() >= std::mem::size_of::<RustBufferC>());
    // SAFETY: slot.len() is checked; RustBufferC is repr(C) so any byte pattern of the
    // right size is a valid representation. read_unaligned handles slot's u8 alignment.
    unsafe { std::ptr::read_unaligned(slot.as_ptr() as *const RustBufferC) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_scalars() {
        let mut buf = [0u8; 8];
        write_u32(&mut buf[..4], 0xDEAD_BEEF);
        assert_eq!(read_u32(&buf[..4]), 0xDEAD_BEEF);

        write_i64(&mut buf, -42);
        assert_eq!(read_i64(&buf), -42);

        write_f64(&mut buf, std::f64::consts::PI);
        assert_eq!(read_f64(&buf), std::f64::consts::PI);
    }

    #[test]
    fn round_trip_pointer() {
        let mut buf = [0u8; std::mem::size_of::<usize>()];
        let p = 0x1234_5678_usize as *const c_void;
        write_pointer(&mut buf, p);
        assert_eq!(read_pointer(&buf), 0x1234_5678);
    }
}
