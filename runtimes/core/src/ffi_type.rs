/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! The abstract type language that drives the entire bridge.

use crate::{Error, Result};

#[derive(Debug, Clone)]
pub enum FfiTypeDesc {
    UInt8,
    Int8,
    UInt16,
    Int16,
    UInt32,
    Int32,
    UInt64,
    Int64,
    Float32,
    Float64,
    Handle,
    RustBuffer,
    ForeignBytes,
    RustCallStatus,
    Callback(String),
    Struct(String),
    Reference(Box<FfiTypeDesc>),
    MutReference(Box<FfiTypeDesc>),
    VoidPointer,
    Void,
}

/// Every tag name in the player vocabulary. `desc_from_name` covers the subset
/// that has a wire encoding; the last three have no wire representation.
pub const ALL_TAG_NAMES: [&str; 20] = [
    "Void",
    "UInt8",
    "Int8",
    "UInt16",
    "Int16",
    "UInt32",
    "Int32",
    "UInt64",
    "Int64",
    "Float32",
    "Float64",
    "Handle",
    "RustBuffer",
    "Callback",
    "Struct",
    "Reference",
    "RustCallStatus",
    "ForeignBytes",
    "VoidPointer",
    "MutReference",
];

/// The tag name for a descriptor. Exhaustive, so a new `FfiTypeDesc` variant
/// fails to compile here until it is named. Adding it to `ALL_TAG_NAMES` is a
/// separate, unforced step: the test comparing the two drives off a hand-written
/// probe list, so a variant absent from both still passes.
pub fn tag_name_of(desc: &FfiTypeDesc) -> &'static str {
    match desc {
        FfiTypeDesc::Void => "Void",
        FfiTypeDesc::UInt8 => "UInt8",
        FfiTypeDesc::Int8 => "Int8",
        FfiTypeDesc::UInt16 => "UInt16",
        FfiTypeDesc::Int16 => "Int16",
        FfiTypeDesc::UInt32 => "UInt32",
        FfiTypeDesc::Int32 => "Int32",
        FfiTypeDesc::UInt64 => "UInt64",
        FfiTypeDesc::Int64 => "Int64",
        FfiTypeDesc::Float32 => "Float32",
        FfiTypeDesc::Float64 => "Float64",
        FfiTypeDesc::Handle => "Handle",
        FfiTypeDesc::RustBuffer => "RustBuffer",
        FfiTypeDesc::Callback(_) => "Callback",
        FfiTypeDesc::Struct(_) => "Struct",
        FfiTypeDesc::Reference(_) => "Reference",
        FfiTypeDesc::RustCallStatus => "RustCallStatus",
        FfiTypeDesc::ForeignBytes => "ForeignBytes",
        FfiTypeDesc::VoidPointer => "VoidPointer",
        FfiTypeDesc::MutReference(_) => "MutReference",
    }
}

/// Build a descriptor from a player tag name and, for the named tags, the type
/// name that travels beside it. The name is what codegen emits and what both
/// bridges read, so this is the single construction point for a wire type.
///
/// Covers 17 of the 20 names in [`ALL_TAG_NAMES`]. `MutReference`,
/// `ForeignBytes` and `VoidPointer` have no wire form; a bridge that marshals
/// them maps them to a wire name itself.
pub fn desc_from_name(tag_name: &str, type_name: Option<&str>) -> Result<FfiTypeDesc> {
    match tag_name {
        "Callback" => type_name
            .map(|n| FfiTypeDesc::Callback(n.to_owned()))
            .ok_or_else(|| Error::UnsupportedType("Callback requires a type name".into())),
        "Struct" => type_name
            .map(|n| FfiTypeDesc::Struct(n.to_owned()))
            .ok_or_else(|| Error::UnsupportedType("Struct requires a type name".into())),
        "Reference" => type_name
            .map(|n| FfiTypeDesc::Reference(Box::new(FfiTypeDesc::Struct(n.to_owned()))))
            .ok_or_else(|| Error::UnsupportedType("Reference requires a struct type name".into())),
        "Void" => Ok(FfiTypeDesc::Void),
        "UInt8" => Ok(FfiTypeDesc::UInt8),
        "Int8" => Ok(FfiTypeDesc::Int8),
        "UInt16" => Ok(FfiTypeDesc::UInt16),
        "Int16" => Ok(FfiTypeDesc::Int16),
        "UInt32" => Ok(FfiTypeDesc::UInt32),
        "Int32" => Ok(FfiTypeDesc::Int32),
        "UInt64" => Ok(FfiTypeDesc::UInt64),
        "Int64" => Ok(FfiTypeDesc::Int64),
        "Float32" => Ok(FfiTypeDesc::Float32),
        "Float64" => Ok(FfiTypeDesc::Float64),
        "Handle" => Ok(FfiTypeDesc::Handle),
        "RustBuffer" => Ok(FfiTypeDesc::RustBuffer),
        "RustCallStatus" => Ok(FfiTypeDesc::RustCallStatus),
        other => Err(Error::UnsupportedType(format!("unknown type tag {other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_tag_names_matches_every_desc_variant() {
        let probes = [
            FfiTypeDesc::Void,
            FfiTypeDesc::UInt8,
            FfiTypeDesc::Int8,
            FfiTypeDesc::UInt16,
            FfiTypeDesc::Int16,
            FfiTypeDesc::UInt32,
            FfiTypeDesc::Int32,
            FfiTypeDesc::UInt64,
            FfiTypeDesc::Int64,
            FfiTypeDesc::Float32,
            FfiTypeDesc::Float64,
            FfiTypeDesc::Handle,
            FfiTypeDesc::RustBuffer,
            FfiTypeDesc::Callback("p".to_owned()),
            FfiTypeDesc::Struct("p".to_owned()),
            FfiTypeDesc::Reference(Box::new(FfiTypeDesc::Void)),
            FfiTypeDesc::RustCallStatus,
            FfiTypeDesc::ForeignBytes,
            FfiTypeDesc::VoidPointer,
            FfiTypeDesc::MutReference(Box::new(FfiTypeDesc::Void)),
        ];
        let mut seen: Vec<&str> = probes.iter().map(tag_name_of).collect();
        seen.sort_unstable();
        seen.dedup();
        let mut all = ALL_TAG_NAMES.to_vec();
        all.sort_unstable();
        assert_eq!(seen, all, "ALL_TAG_NAMES vs FfiTypeDesc variants");
    }

    #[test]
    fn desc_from_name_covers_exactly_the_wire_names() {
        for name in ALL_TAG_NAMES {
            let wire_eligible = !matches!(name, "ForeignBytes" | "VoidPointer" | "MutReference");
            // Named tags need a type name; supply one unconditionally.
            let got = desc_from_name(name, Some("N"));
            assert_eq!(got.is_ok(), wire_eligible, "wire eligibility for {name}");
            if let Ok(d) = got {
                assert_eq!(tag_name_of(&d), name, "identity for {name}");
            }
        }
        assert!(desc_from_name("NotATag", None).is_err());
    }

    #[test]
    fn desc_from_name_requires_a_type_name_for_named_tags() {
        for name in ["Callback", "Struct", "Reference"] {
            assert!(desc_from_name(name, None).is_err(), "{name} without a name");
        }
    }
}
