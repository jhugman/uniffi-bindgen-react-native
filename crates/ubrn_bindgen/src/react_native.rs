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
    bindings::{gen_cpp, gen_typescript, metadata::ModuleMetadata, type_map::TypeMap},
    switches::SwitchArgs,
};

pub(crate) struct ReactNativeBindingGenerator {
    switches: SwitchArgs,
    ts_dir: Utf8PathBuf,
    cpp_dir: Utf8PathBuf,
}

impl ReactNativeBindingGenerator {
    pub(crate) fn new(ts_dir: Utf8PathBuf, cpp_dir: Utf8PathBuf, switches: SwitchArgs) -> Self {
        Self {
            ts_dir,
            cpp_dir,
            switches,
        }
    }

    fn generate_ts(
        components: &[Component<ReactNativeConfig>],
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

            let lowlevel_ts = gen_typescript::generate_lowlevel_code(&component.ci, &module)?;
            let lowlevel_ts_path = out_dir.join(module.ts_ffi_filename());
            ubrn_common::write_file(lowlevel_ts_path, lowlevel_ts)?;
        }
        if try_format_code {
            gen_typescript::format_directory(out_dir)?;
        }
        Ok(())
    }

    fn generate_cpp(
        components: &[Component<ReactNativeConfig>],
        out_dir: &Utf8PathBuf,
        try_format_code: bool,
    ) -> Result<(), anyhow::Error> {
        for component in components {
            let module = ModuleMetadata::from(component);

            let cpp = gen_cpp::generate_cpp(&component.ci, &component.config.cpp, &module)?;
            let cpp_path = out_dir.join(module.cpp_filename());
            ubrn_common::write_file(cpp_path, cpp)?;

            let hpp = gen_cpp::generate_hpp(&component.ci, &component.config.cpp, &module)?;
            let hpp_path = out_dir.join(module.hpp_filename());
            ubrn_common::write_file(hpp_path, hpp)?;
        }
        if try_format_code {
            gen_cpp::format_directory(out_dir)?;
        }
        Ok(())
    }
}

impl BindingGenerator for ReactNativeBindingGenerator {
    type Config = ReactNativeConfig;

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
        Self::generate_cpp(components, &self.cpp_dir, settings.try_format_code)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ReactNativeConfig {
    #[serde(default, alias = "javascript", alias = "js", alias = "ts")]
    pub(crate) typescript: gen_typescript::Config,
    #[serde(default)]
    pub(crate) cpp: gen_cpp::Config,
}
