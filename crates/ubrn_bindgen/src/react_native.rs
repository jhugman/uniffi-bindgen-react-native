/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::fs;

use anyhow::Result;
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use uniffi_bindgen::{BindingGenerator, Component, GenerationSettings};

use crate::{
    bindings::{
        gen_cpp::{self, CppBindings},
        gen_typescript::{self, TsBindings},
        metadata::ModuleMetadata,
        type_map::TypeMap,
    },
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

    pub(crate) fn format_code(&self) -> Result<()> {
        gen_typescript::format_directory(&self.ts_dir)?;
        gen_cpp::format_directory(&self.cpp_dir)?;
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
        let type_map = TypeMap::from(components);

        let out_dir = &self.ts_dir;
        for component in components {
            let ci = &component.ci;
            let module = ModuleMetadata::from(component);
            let config = &component.config;
            let TsBindings { codegen, frontend } = gen_typescript::generate_bindings(
                ci,
                &config.typescript,
                &module,
                &self.switches,
                &type_map,
            )?;

            let codegen_path = out_dir.join(module.ts_ffi_filename());
            fs::write(codegen_path, codegen)?;

            let frontend_path = out_dir.join(module.ts_filename());
            fs::write(frontend_path, frontend)?;
        }

        let out_dir = &self.cpp_dir;
        for component in components {
            let ci = &component.ci;
            let module: ModuleMetadata = component.into();
            let config = &component.config;

            let CppBindings { hpp, cpp } = gen_cpp::generate_bindings(ci, &config.cpp, &module)?;
            let cpp_path = out_dir.join(module.cpp_filename());
            let hpp_path = out_dir.join(module.hpp_filename());
            fs::write(cpp_path, cpp)?;
            fs::write(hpp_path, hpp)?;
        }
        if settings.try_format_code {
            self.format_code()?;
        }
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
