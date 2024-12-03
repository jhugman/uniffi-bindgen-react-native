/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::fs;

use anyhow::Result;
use camino::Utf8Path;
use uniffi_bindgen::{BindingGenerator, Component, GenerationSettings};

use ubrn_common::{fmt, run_cmd_quietly};

use crate::bindings::{
    gen_cpp::{self, CppBindings},
    gen_typescript::{self, TsBindings},
    metadata::ModuleMetadata,
    type_map::TypeMap,
    uniffi_toml::ReactNativeConfig,
    OutputArgs,
};

pub(crate) struct ReactNativeBindingGenerator {
    output: OutputArgs,
}

impl ReactNativeBindingGenerator {
    pub(crate) fn new(output: OutputArgs) -> Self {
        Self { output }
    }

    pub(crate) fn format_code(&self) -> Result<()> {
        format_ts(&self.output.ts_dir.canonicalize_utf8()?)?;
        format_cpp(&self.output.cpp_dir.canonicalize_utf8()?)?;
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
        let mut type_map = TypeMap::default();
        for component in components {
            type_map.insert_ci(&component.ci);
        }
        for component in components {
            let ci = &component.ci;
            let module: ModuleMetadata = component.into();
            let config = &component.config;
            let TsBindings { codegen, frontend } =
                gen_typescript::generate_bindings(ci, &config.typescript, &module, &type_map)?;

            let out_dir = &self.output.ts_dir.canonicalize_utf8()?;
            let codegen_path = out_dir.join(module.ts_ffi_filename());
            let frontend_path = out_dir.join(module.ts_filename());
            fs::write(codegen_path, codegen)?;
            fs::write(frontend_path, frontend)?;

            let out_dir = &self.output.cpp_dir.canonicalize_utf8()?;
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

fn format_ts(out_dir: &Utf8Path) -> Result<()> {
    if let Some(mut prettier) = fmt::prettier(out_dir, false)? {
        run_cmd_quietly(&mut prettier)?
    } else {
        eprintln!("No prettier found. Install with `yarn add --dev prettier`");
    }
    Ok(())
}

fn format_cpp(out_dir: &Utf8Path) -> Result<()> {
    if let Some(mut clang_format) = fmt::clang_format(out_dir, false)? {
        run_cmd_quietly(&mut clang_format)?
    } else {
        eprintln!("Skipping formatting C++. Is clang-format installed?");
    }
    Ok(())
}
