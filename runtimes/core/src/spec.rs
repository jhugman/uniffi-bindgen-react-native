/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use crate::FfiTypeDesc;
use std::collections::HashMap;

/// A single field in a struct definition.
#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub field_type: FfiTypeDesc,
}

/// A parsed struct definition (list of fields).
#[derive(Debug, Clone)]
pub struct StructDef {
    pub fields: Vec<StructField>,
}

/// An alias for the common type used to pass struct definitions to CIF builders.
pub type StructDefs = HashMap<String, StructDef>;

/// Describes the signature of a Rust FFI function exported by a UniFFI library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDef {
    pub args: Vec<FfiTypeDesc>,
    pub ret: FfiTypeDesc,
    pub has_rust_call_status: bool,
}

/// Describes the signature of a callback interface method that JS must implement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackDef {
    pub args: Vec<FfiTypeDesc>,
    pub ret: FfiTypeDesc,
    pub has_rust_call_status: bool,
    pub out_return: bool,
}

impl CallbackDef {
    /// True when nothing flows back to the caller, so a call from another
    /// thread can be posted without waiting for JS to run it.
    pub fn is_fire_and_forget(&self) -> bool {
        use FfiTypeDesc::*;
        let scalar = |t: &FfiTypeDesc| {
            matches!(
                t,
                UInt8
                    | Int8
                    | UInt16
                    | Int16
                    | UInt32
                    | Int32
                    | UInt64
                    | Int64
                    | Float32
                    | Float64
                    | Handle
            )
        };
        self.ret == Void
            && !self.has_rust_call_status
            && !self.out_return
            && self.args.iter().all(scalar)
    }
}

/// The three RustBuffer lifecycle symbols that every UniFFI library exports.
#[derive(Debug, Clone)]
pub struct RustBufferSymbols {
    pub alloc: String,
    pub free: String,
    pub from_bytes: String,
}

/// Complete specification for a loaded UniFFI module.
#[derive(Debug, Clone)]
pub struct ModuleSpec {
    pub rustbuffer_symbols: RustBufferSymbols,
    pub functions: HashMap<String, FunctionDef>,
    pub callbacks: HashMap<String, CallbackDef>,
    pub structs: HashMap<String, StructDef>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use FfiTypeDesc::*;

    fn def(args: Vec<FfiTypeDesc>, ret: FfiTypeDesc, status: bool, out: bool) -> CallbackDef {
        CallbackDef {
            args,
            ret,
            has_rust_call_status: status,
            out_return: out,
        }
    }

    #[test]
    fn is_fire_and_forget() {
        assert!(def(vec![UInt64, Int8], Void, false, false).is_fire_and_forget());
        assert!(def(vec![Handle, Int32, Float64], Void, false, false).is_fire_and_forget());
        assert!(!def(vec![Handle], Void, true, false).is_fire_and_forget());
        assert!(!def(vec![Handle], Void, false, true).is_fire_and_forget());
        assert!(!def(vec![Handle], Int32, false, false).is_fire_and_forget());
        assert!(!def(vec![RustBuffer], Void, false, false).is_fire_and_forget());
        assert!(!def(vec![Callback("X".into())], Void, false, false).is_fire_and_forget());
        // Each of these points at the caller's memory, which a posted call would read
        // after the caller has returned.
        let s = || Box::new(Struct("S".into()));
        for arg in [
            Reference(s()),
            MutReference(s()),
            VoidPointer,
            Struct("S".into()),
            ForeignBytes,
        ] {
            assert!(
                !def(vec![Handle, arg.clone()], Void, false, false).is_fire_and_forget(),
                "{arg:?} must not be posted"
            );
        }
    }
}
