/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod gen_cpp;
mod gen_typescript;

use std::{collections::HashMap, fs, process::Command};

use anyhow::Result;
use camino::Utf8Path;
use extend::ext;
use heck::ToUpperCamelCase;
use serde::Deserialize;
use uniffi_bindgen::{
    interface::{FfiArgument, FfiFunction, FfiType, Function},
    BindingGenerator, BindingsConfig, ComponentInterface,
};
use uniffi_common::{resolve, run_cmd_quietly};

use self::{gen_cpp::CppBindings, gen_typescript::TsBindings};

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
        let namespace = ci.namespace();
        let TsBindings { codegen, frontend } = gen_typescript::generate_bindings(ci, config)?;
        let codegen_file = format!("{}.ts", &config.ffi_ts_filename);
        let codegen_path = out_dir.join(codegen_file);
        let frontend_path = out_dir.join(format!("{namespace}.ts"));

        fs::write(codegen_path, codegen)?;
        fs::write(frontend_path, frontend)?;

        if try_format_code {
            let _ = format_ts(out_dir);
        }

        let CppBindings { hpp, cpp } = gen_cpp::generate_bindings(ci, config)?;
        let cpp_path = out_dir.join(format!("{namespace}.cpp"));
        let hpp_path = out_dir.join(format!("{namespace}.hpp"));

        fs::write(&cpp_path, cpp)?;
        fs::write(&hpp_path, hpp)?;
        if try_format_code {
            let _ = format_cpp(out_dir, &[&cpp_path, &hpp_path]);
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

fn format_cpp(out_dir: &Utf8Path, files: &[&Utf8Path]) -> Result<()> {
    let result = run_cmd_quietly(
        Command::new("clang-format")
            .current_dir(out_dir)
            .arg("-i")
            .arg("--style=file")
            .arg("--fallback-style=LLVM")
            .args(files),
    );
    if result.is_err() {
        eprintln!("Could not format C++ code. Is `clang-format` installed?");
    }
    result
}

#[ext]
impl ComponentInterface {
    fn ffi_function_string_to_arraybuffer(&self) -> FfiFunction {
        let meta = uniffi_meta::FnMetadata {
            module_path: "internal".to_string(),
            name: "ffi__string_to_arraybuffer".to_owned(),
            is_async: false,
            inputs: Default::default(),
            return_type: None,
            throws: None,
            checksum: None,
            docstring: None,
        };
        let func: Function = meta.into();
        let mut ffi = func.ffi_func().clone();
        ffi.init(
            Some(FfiType::ForeignBytes),
            vec![FfiArgument::new("string", FfiType::RustBuffer(None))],
        );
        ffi.clone()
    }

    fn ffi_function_arraybuffer_to_string(&self) -> FfiFunction {
        let meta = uniffi_meta::FnMetadata {
            module_path: "internal".to_string(),
            name: "ffi__arraybuffer_to_string".to_owned(),
            is_async: false,
            inputs: Default::default(),
            return_type: None,
            throws: None,
            checksum: None,
            docstring: None,
        };
        let func: Function = meta.into();
        let mut ffi = func.ffi_func().clone();
        ffi.init(
            Some(FfiType::RustBuffer(None)),
            vec![FfiArgument::new("buffer", FfiType::ForeignBytes)],
        );
        ffi.clone()
    }

    fn ffi_function_string_to_bytelength(&self) -> FfiFunction {
        let meta = uniffi_meta::FnMetadata {
            module_path: "internal".to_string(),
            name: "ffi__string_to_byte_length".to_owned(),
            is_async: false,
            inputs: Default::default(),
            return_type: None,
            throws: None,
            checksum: None,
            docstring: None,
        };
        let func: Function = meta.into();
        let mut ffi = func.ffi_func().clone();
        ffi.init(
            Some(FfiType::Int32),
            vec![FfiArgument::new("string", FfiType::RustBuffer(None))],
        );
        ffi.clone()
    }

    fn iter_ffi_functions_js_to_cpp_and_back(&self) -> impl Iterator<Item = FfiFunction> {
        vec![
            self.ffi_function_string_to_bytelength(),
            self.ffi_function_string_to_arraybuffer(),
            self.ffi_function_arraybuffer_to_string(),
        ]
        .into_iter()
    }

    fn iter_ffi_functions_js_to_cpp(&self) -> impl Iterator<Item = FfiFunction> {
        self.iter_ffi_functions_js_to_cpp_and_back()
            .chain(self.iter_ffi_functions_js_to_rust())
    }

    fn iter_ffi_functions_js_to_rust(&self) -> impl Iterator<Item = FfiFunction> {
        self.iter_ffi_function_definitions().filter(|f| {
            let name = f.name();
            !name.contains("_rust_future_") && !name.contains("_rustbuffer_")
        })
    }

    fn iter_ffi_functions_cpp_to_rust(&self) -> impl Iterator<Item = FfiFunction> {
        self.iter_ffi_functions_js_to_rust()
    }
}

#[ext]
impl FfiFunction {
    fn is_internal(&self) -> bool {
        self.name().contains("ffi__")
    }
}

#[ext]
impl FfiType {
    fn requires_argument_cleanup(&self) -> bool {
        // If this returns true, there is expected a Bridging<{{ self|ffi_type_name() }}>.argument_cleanup(v).
        match self {
            Self::RustBuffer(_) => true, // includes/RustBuffer.h
            _ => false,
        }
    }
}
