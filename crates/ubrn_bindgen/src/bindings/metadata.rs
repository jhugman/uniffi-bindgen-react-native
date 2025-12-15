/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use uniffi_bindgen::Component;

#[derive(Default)]
pub struct ModuleMetadata {
    pub(crate) namespace: String,
}

impl ModuleMetadata {
    pub fn new(namespace: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
        }
    }

    pub fn cpp_module(&self) -> String {
        // Explicitly capitalize the first letter to ensure proper casing
        let mut chars = self.namespace.chars();
        let first_upper = chars.next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
        let rest: String = chars.collect();
        format!("Native{}{}", first_upper, rest)
    }

    pub fn cpp_filename(&self) -> String {
        format!("{}.cpp", self.namespace)
    }

    pub fn hpp_filename(&self) -> String {
        format!("{}.hpp", self.namespace)
    }

    #[cfg(feature = "wasm")]
    pub fn rs_filename(&self) -> String {
        format!("{}.rs", self.rs_module())
    }

    #[cfg(feature = "wasm")]
    pub fn rs_module(&self) -> String {
        format!("{}_module", self.namespace)
    }

    pub fn ts(&self) -> String {
        self.namespace.clone()
    }

    pub fn ts_filename(&self) -> String {
        format!("{}.ts", self.ts())
    }

    pub fn ts_ffi(&self) -> String {
        format!("{}-ffi", self.namespace)
    }

    pub fn ts_ffi_filename(&self) -> String {
        format!("{}.ts", self.ts_ffi())
    }
}

impl<T> From<&Component<T>> for ModuleMetadata {
    fn from(value: &Component<T>) -> Self {
        let namespace = value.ci.namespace().to_string();
        Self { namespace }
    }
}
