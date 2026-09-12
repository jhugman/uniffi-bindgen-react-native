/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use uniffi_bindgen::pipeline::general;

/// Map a `general::FfiType` to the player's `FfiType.XXX` expression string.
///
/// These correspond to the tags defined in `runtimes/napi/lib.js`.
pub(super) fn ffi_type_to_player(ffi_type: &general::FfiType) -> String {
    match ffi_type {
        general::FfiType::UInt8 => "FfiType.UInt8".into(),
        general::FfiType::Int8 => "FfiType.Int8".into(),
        general::FfiType::UInt16 => "FfiType.UInt16".into(),
        general::FfiType::Int16 => "FfiType.Int16".into(),
        general::FfiType::UInt32 => "FfiType.UInt32".into(),
        general::FfiType::Int32 => "FfiType.Int32".into(),
        general::FfiType::UInt64 => "FfiType.UInt64".into(),
        general::FfiType::Int64 => "FfiType.Int64".into(),
        general::FfiType::Float32 => "FfiType.Float32".into(),
        general::FfiType::Float64 => "FfiType.Float64".into(),
        general::FfiType::Handle(_) => "FfiType.Handle".into(),
        general::FfiType::RustBuffer(_) => "FfiType.RustBuffer".into(),
        general::FfiType::RustCallStatus => "FfiType.RustCallStatus".into(),
        general::FfiType::ForeignBytes => "FfiType.ForeignBytes".into(),
        general::FfiType::VoidPointer => "FfiType.VoidPointer".into(),
        general::FfiType::Function(name) => {
            format!("FfiType.Callback(\"{}\")", name.0)
        }
        general::FfiType::Struct(name) => {
            format!("FfiType.Struct(\"{}\")", name.0)
        }
        general::FfiType::Reference(inner) => {
            format!("FfiType.Reference({})", ffi_type_to_player(inner))
        }
        general::FfiType::MutReference(inner) => {
            format!("FfiType.MutReference({})", ffi_type_to_player(inner))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uniffi_bindgen::pipeline::general::{FfiFunctionTypeName, FfiStructName, HandleKind};

    /// Pull the tag name out of an emitted `"FfiType.XXX"` or
    /// `"FfiType.XXX(...)"` expression, e.g. `"FfiType.UInt8"` -> `"UInt8"`.
    fn tag_name(emitted: &str) -> &str {
        emitted
            .strip_prefix("FfiType.")
            .unwrap_or_else(|| panic!("{emitted:?} doesn't start with \"FfiType.\""))
            .split('(')
            .next()
            .unwrap()
    }

    #[test]
    fn every_emitted_tag_name_is_one_core_can_name() {
        let probes = [
            general::FfiType::UInt8,
            general::FfiType::Int8,
            general::FfiType::UInt16,
            general::FfiType::Int16,
            general::FfiType::UInt32,
            general::FfiType::Int32,
            general::FfiType::UInt64,
            general::FfiType::Int64,
            general::FfiType::Float32,
            general::FfiType::Float64,
            general::FfiType::Handle(HandleKind::RustFuture),
            general::FfiType::RustBuffer(None),
            general::FfiType::RustCallStatus,
            general::FfiType::ForeignBytes,
            general::FfiType::VoidPointer,
            general::FfiType::Function(FfiFunctionTypeName("cb".into())),
            general::FfiType::Struct(FfiStructName("s".into())),
            general::FfiType::Reference(Box::new(general::FfiType::UInt8)),
            general::FfiType::MutReference(Box::new(general::FfiType::UInt8)),
        ];

        // Tags core names but `desc_from_name` cannot build, so each bridge
        // resolves them itself: napi maps all three directly, and the jsi shim
        // rewrites the first two into `Reference`/`Handle`. `ForeignBytes` has
        // no shim rewrite, so emitting it would fail a jsi registration —
        // hence naming the exceptions here rather than accepting any of the 20.
        const BRIDGE_RESOLVED: [&str; 3] = ["MutReference", "VoidPointer", "ForeignBytes"];

        for probe in &probes {
            let emitted = ffi_type_to_player(probe);
            let name = tag_name(&emitted);
            assert!(
                uniffi_runtime_core::ALL_TAG_NAMES.contains(&name),
                "ffi_type_to_player({probe:?}) emitted {emitted:?}, whose tag {name:?} core doesn't know"
            );
            if BRIDGE_RESOLVED.contains(&name) {
                continue;
            }
            assert!(
                uniffi_runtime_core::desc_from_name(name, Some("p")).is_ok(),
                "ffi_type_to_player({probe:?}) emitted {emitted:?}, whose tag {name:?} \
                 core cannot resolve and no bridge special-cases"
            );
        }
    }
}
