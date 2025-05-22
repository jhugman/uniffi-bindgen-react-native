/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use anyhow::Result;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use uniffi_bindgen::{BindingGenerator, Component, GenerationSettings};

use crate::{
    bindings::{gen_rust, gen_typescript, metadata::ModuleMetadata, type_map::TypeMap},
    switches::SwitchArgs,
};

pub(crate) struct WasmBindingGenerator {
    switches: SwitchArgs,
    ts_dir: Utf8PathBuf,
    rust_dir: Utf8PathBuf,
}

impl WasmBindingGenerator {
    pub(crate) fn new(ts_dir: Utf8PathBuf, rust_dir: Utf8PathBuf, switches: SwitchArgs) -> Self {
        Self {
            ts_dir,
            rust_dir,
            switches,
        }
    }

    fn generate_ts(
        components: &[Component<WasmConfig>],
        switches: &SwitchArgs,
        out_dir: &Utf8PathBuf,
        try_format_code: bool,
    ) -> std::result::Result<(), anyhow::Error> {
        let type_map = TypeMap::from(components);
        for component in components {
            let module = ModuleMetadata::from(component);

            let api_ts = gen_typescript::generate_api_code(
                &component.ci,
                &component.config.typescript,
                &module,
                switches,
                &type_map,
            )?;
            let api_ts_path = out_dir.join(module.ts_filename());
            ubrn_common::write_file(api_ts_path, api_ts)?;
        }
        if try_format_code {
            gen_typescript::format_directory(out_dir)?;
        }
        Ok(())
    }

    fn generate_rs(
        components: &[Component<WasmConfig>],
        switches: &SwitchArgs,
        out_dir: &Utf8PathBuf,
        try_format_code: bool,
    ) -> Result<(), anyhow::Error> {
        for component in components {
            let module = ModuleMetadata::from(component);

            let rs_code = gen_rust::generate_rs(
                &component.ci,
                &module,
                &component.config.rust,
                switches,
                try_format_code,
            )?;
            let rs_path = out_dir.join(module.rs_filename());
            ubrn_common::write_file(rs_path, rs_code)?;
        }
        Ok(())
    }
}

impl BindingGenerator for WasmBindingGenerator {
    type Config = WasmConfig;

    fn new_config(&self, root_toml: &toml::value::Value) -> Result<Self::Config> {
        Ok(match root_toml.get("bindings") {
            Some(v) => v.clone().try_into()?,
            None => Default::default(),
        })
    }

    fn update_component_configs(
        &self,
        _settings: &GenerationSettings,
        _components: &mut Vec<Component<Self::Config>>,
    ) -> Result<()> {
        // NOOP
        Ok(())
    }

    fn write_bindings(
        &self,
        settings: &GenerationSettings,
        components: &[Component<Self::Config>],
    ) -> Result<()> {
        Self::generate_ts(
            components,
            &self.switches,
            &self.ts_dir,
            settings.try_format_code,
        )?;
        Self::generate_rs(
            components,
            &self.switches,
            &self.rust_dir,
            settings.try_format_code,
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct WasmConfig {
    #[serde(default, alias = "javascript", alias = "js", alias = "ts")]
    pub(crate) typescript: gen_typescript::Config,

    #[serde(default, alias = "rs")]
    pub(crate) rust: gen_rust::Config,
}
