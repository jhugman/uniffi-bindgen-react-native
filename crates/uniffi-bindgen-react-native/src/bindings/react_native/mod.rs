mod gen_typescript;

use std::{collections::HashMap, fs, process::Command};

use anyhow::Result;
use camino::Utf8Path;
use heck::ToUpperCamelCase;
use serde::Deserialize;
use uniffi_bindgen::{BindingGenerator, BindingsConfig, ComponentInterface};
use uniffi_common::{resolve, run_cmd_quietly};

use self::gen_typescript::TsBindings;

#[derive(Deserialize)]
pub(crate) struct ReactNativeConfig {
    #[serde(default)]
    use_codegen: bool,

    #[serde(default)]
    cpp_module: String,

    #[serde(default)]
    ffi_ts_filename: String,
}

impl BindingsConfig for ReactNativeConfig {
    fn update_from_ci(&mut self, ci: &ComponentInterface) {
        let ns = ci.namespace();
        let cpp_module = format!("Native{}", ns.to_upper_camel_case());
        self.ffi_ts_filename = if self.use_codegen {
            format!("Native{cpp_module}")
        } else {
            format!("{ns}-ffi")
        };
        self.cpp_module = cpp_module;
    }

    fn update_from_cdylib_name(&mut self, _cdylib_name: &str) {
        // NOOP
    }

    fn update_from_dependency_configs(&mut self, _config_map: HashMap<&str, &Self>) {
        // NOOP
    }
}

pub(crate) struct ReactNativeBindingGenerator;

impl BindingGenerator for ReactNativeBindingGenerator {
    type Config = ReactNativeConfig;

    fn write_bindings(
        &self,
        ci: &ComponentInterface,
        config: &Self::Config,
        out_dir: &Utf8Path,
        try_format_code: bool,
    ) -> Result<()> {
        let TsBindings { codegen, frontend } =
            gen_typescript::generate_frontend_bindings(ci, config, try_format_code)?;
        let codegen_file = format!("{}.ts", &config.ffi_ts_filename);
        let codegen_path = out_dir.join(codegen_file);
        let frontend_path = out_dir.join(format!("{}.ts", ci.namespace()));

        fs::write(codegen_path, codegen)?;
        fs::write(frontend_path, frontend)?;

        if try_format_code {
            format_ts(out_dir)?;
        }

        Ok(())
    }

    fn check_library_path(
        &self,
        _library_path: &Utf8Path,
        _cdylib_name: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }
}

fn format_ts(out_dir: &Utf8Path) -> Result<()> {
    if let Some(prettier) = resolve(out_dir, "node_modules/.bin/prettier")? {
        run_cmd_quietly(
            Command::new(prettier)
                .arg(".")
                .arg("--write")
                .current_dir(out_dir),
        )?;
    } else {
        eprintln!("No prettier found. Install with `yarn add --dev prettier`");
    }
    Ok(())
}
