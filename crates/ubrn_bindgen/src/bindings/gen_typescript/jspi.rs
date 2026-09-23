// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::collections::HashSet;

use anyhow::{ensure, Result};
use heck::ToUpperCamelCase;
use uniffi_bindgen::pipeline::general;

use super::config::AsyncSelection;

struct Target<'a> {
    name: &'a str,
    has_callbacks: bool,
    // Raw ABI symbol and optional shared UniFFI poll symbol.
    exports: Vec<(&'a str, Option<&'a str>)>,
}

/// Apply the public callable selection with wasm2 ABI eligibility checks.
pub(super) fn resolve_wasm2(
    selection: &AsyncSelection,
    namespace: &general::Namespace,
) -> Result<HashSet<String>> {
    let eligible: HashSet<_> = namespace
        .ffi_definitions
        .iter()
        .filter_map(|def| {
            let general::FfiDefinition::RustFunction(f) = def else {
                return None;
            };
            let supported = |ty: &general::FfiType| {
                matches!(
                    ty,
                    general::FfiType::UInt8
                        | general::FfiType::Int8
                        | general::FfiType::UInt16
                        | general::FfiType::Int16
                        | general::FfiType::UInt32
                        | general::FfiType::Int32
                        | general::FfiType::UInt64
                        | general::FfiType::Int64
                        | general::FfiType::Float32
                        | general::FfiType::Float64
                        | general::FfiType::Handle(_)
                        | general::FfiType::RustBuffer(_)
                )
            };
            (f.arguments.iter().all(|arg| supported(&arg.ty.ty))
                && f.return_type
                    .ty
                    .as_ref()
                    .is_none_or(|ret| supported(&ret.ty)))
            .then_some(f.name.0.as_str())
        })
        .collect();
    let mut targets = namespace_targets(namespace);
    for target in &mut targets {
        target.has_callbacks |= target
            .exports
            .iter()
            .any(|(symbol, _)| !eligible.contains(symbol));
    }
    select_wasm2(selection, &targets)
}

fn select_wasm2(selection: &AsyncSelection, targets: &[Target<'_>]) -> Result<HashSet<String>> {
    select(selection, targets, true)
        .map_err(|e| anyhow::anyhow!("wasm2 JSPI selection contains an unsupported callable: {e}"))
}

impl<'a> Target<'a> {
    fn type_(
        name: &'a str,
        constructors: &'a [general::Constructor],
        methods: &'a [general::Method],
        traits: &'a general::UniffiTraitMethods,
        has_callbacks: bool,
    ) -> Self {
        let trait_methods = [
            &traits.display_fmt,
            &traits.debug_fmt,
            &traits.eq_eq,
            &traits.eq_ne,
            &traits.hash_hash,
            &traits.ord_cmp,
        ];
        let exports = constructors
            .iter()
            .map(|c| &c.callable)
            .chain(methods.iter().map(|m| &m.callable))
            .chain(
                trait_methods
                    .into_iter()
                    .filter_map(|m| m.as_ref())
                    .map(|m| &m.callable),
            )
            .map(|c| {
                (
                    c.ffi_func.0.as_str(),
                    c.async_data
                        .as_ref()
                        .map(|a| a.ffi_rust_future_poll.0.as_str()),
                )
            })
            .collect();
        Self {
            name,
            has_callbacks,
            exports,
        }
    }
}

/// Resolve the same public callable allowlist for both the Rust and TS generators.
/// Never select clone/free, checksums, allocation, registration or vtable entries.
pub(super) fn resolve(
    selection: &AsyncSelection,
    namespace: &general::Namespace,
    is_web: bool,
) -> Result<HashSet<String>> {
    select(selection, &namespace_targets(namespace), is_web)
}

fn namespace_targets(namespace: &general::Namespace) -> Vec<Target<'_>> {
    let mut targets: Vec<_> = namespace
        .functions
        .iter()
        .map(|f| Target {
            name: &f.callable.name,
            has_callbacks: false,
            exports: vec![(
                &f.callable.ffi_func.0,
                f.callable
                    .async_data
                    .as_ref()
                    .map(|a| a.ffi_rust_future_poll.0.as_str()),
            )],
        })
        .collect();
    for ty in &namespace.type_definitions {
        let target = match ty {
            general::TypeDefinition::Interface(i) => Target::type_(
                &i.name,
                &i.constructors,
                &i.methods,
                &i.uniffi_trait_methods,
                i.imp.has_callback_interface(),
            ),
            general::TypeDefinition::Record(r) => Target::type_(
                &r.name,
                &r.constructors,
                &r.methods,
                &r.uniffi_trait_methods,
                false,
            ),
            general::TypeDefinition::Enum(e) => Target::type_(
                &e.name,
                &e.constructors,
                &e.methods,
                &e.uniffi_trait_methods,
                false,
            ),
            general::TypeDefinition::CallbackInterface(c) => Target {
                name: &c.name,
                has_callbacks: true,
                exports: vec![],
            },
            _ => continue,
        };
        targets.push(target);
    }
    targets
}

fn select(
    selection: &AsyncSelection,
    targets: &[Target<'_>],
    is_web: bool,
) -> Result<HashSet<String>> {
    let enabled = match selection {
        AsyncSelection::All(value) => *value,
        AsyncSelection::Named(names) => !names.is_empty(),
    };
    ensure!(
        !enabled || is_web,
        "jspi is supported only by the web (wasm) and wasm2 backends"
    );
    if let AsyncSelection::Named(names) = selection {
        for name in names {
            let matches: Vec<_> = targets
                .iter()
                .filter(|t| t.name.to_upper_camel_case() == name.to_upper_camel_case())
                .collect();
            ensure!(matches.len() == 1,
                "jspi selection `{name}` must identify one top-level function or object/record/enum type; individual methods and constructors are selected by their owning type");
            let target = matches[0];
            ensure!(!target.has_callbacks,
                "jspi selection `{name}` is callback-capable; suspending inbound callback interfaces is not supported");
        }
    }
    Ok(targets
        .iter()
        .filter(|t| !t.has_callbacks && selection.is_forced(t.name))
        .flat_map(|t| &t.exports)
        .flat_map(|(symbol, poll)| {
            std::iter::once((*symbol).to_owned()).chain(poll.map(|name| format!("{name}_jspi")))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets() -> Vec<Target<'static>> {
        vec![
            Target {
                name: "compute_value",
                has_callbacks: false,
                exports: vec![("ffi_compute", None)],
            },
            Target {
                name: "future",
                has_callbacks: false,
                exports: vec![("ffi_future", Some("ffi_poll_u32"))],
            },
            Target {
                name: "Processor",
                has_callbacks: false,
                exports: vec![
                    ("ffi_new", None),
                    ("ffi_method", None),
                    ("ffi_display", None),
                    ("ffi_async_method", Some("ffi_poll_u32")),
                ],
            },
            Target {
                name: "Listener",
                has_callbacks: true,
                exports: vec![("ffi_callback", None)],
            },
        ]
    }

    fn named(name: &str) -> AsyncSelection {
        AsyncSelection::Named(vec![name.into()])
    }

    #[test]
    fn wasm2_selects_async_and_type_callables_and_rejects_callbacks() {
        let mut top = targets();
        top.truncate(2); // the namespace contributes functions only
        assert_eq!(
            select_wasm2(&AsyncSelection::All(true), &top).unwrap(),
            ["ffi_compute", "ffi_future", "ffi_poll_u32_jspi"]
                .map(str::to_owned)
                .into()
        );
        assert_eq!(
            select_wasm2(&named("computeValue"), &top).unwrap(),
            ["ffi_compute".to_owned()].into()
        );
        assert_eq!(
            select_wasm2(&named("future"), &top).unwrap(),
            ["ffi_future", "ffi_poll_u32_jspi"]
                .map(str::to_owned)
                .into()
        );
        assert_eq!(
            select_wasm2(&named("Processor"), &targets()).unwrap(),
            [
                "ffi_new",
                "ffi_method",
                "ffi_display",
                "ffi_async_method",
                "ffi_poll_u32_jspi"
            ]
            .map(str::to_owned)
            .into()
        );
        assert!(select_wasm2(&named("Listener"), &targets()).is_err());
        top.truncate(1);
        top[0].has_callbacks = true;
        assert!(select_wasm2(&AsyncSelection::All(true), &top)
            .unwrap()
            .is_empty());
        assert!(select_wasm2(&named("computeValue"), &top).is_err());
    }

    #[test]
    fn selection_includes_dedicated_async_poll_adapters() {
        assert_eq!(
            select(&named("processor"), &targets(), true).unwrap(),
            [
                "ffi_new",
                "ffi_method",
                "ffi_display",
                "ffi_async_method",
                "ffi_poll_u32_jspi"
            ]
            .map(str::to_owned)
            .into()
        );
        assert_eq!(
            select(&named("future"), &targets(), true).unwrap(),
            ["ffi_future".to_owned(), "ffi_poll_u32_jspi".to_owned()].into()
        );
        assert_eq!(
            select(&named("computeValue"), &targets(), true).unwrap(),
            ["ffi_compute".to_owned()].into()
        );
        assert_eq!(
            select(&AsyncSelection::All(true), &targets(), true).unwrap(),
            [
                "ffi_compute",
                "ffi_new",
                "ffi_method",
                "ffi_display",
                "ffi_future",
                "ffi_async_method",
                "ffi_poll_u32_jspi"
            ]
            .map(str::to_owned)
            .into()
        );
    }

    #[test]
    fn invalid_or_unsupported_selection_is_an_error() {
        for (name, message) in [
            ("Listener", "callback-capable"),
            ("Processor.new", "owning type"),
            ("typo", "top-level function"),
        ] {
            assert!(select(&named(name), &targets(), true)
                .unwrap_err()
                .to_string()
                .contains(message));
        }
        assert!(select(&AsyncSelection::All(true), &targets(), false)
            .unwrap_err()
            .to_string()
            .contains("web"));
    }

    #[test]
    fn disabled_and_force_async_only_do_not_select_jspi_exports() {
        for setting in ["", "forceAsync = true", "jspi = false", "jspi = []"] {
            let config: super::super::Config = toml::from_str(setting).unwrap();
            assert!(select(&config.jspi, &targets(), false).unwrap().is_empty());
        }
    }
}
