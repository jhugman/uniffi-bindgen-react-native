/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::str::FromStr;

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{Metadata, MetadataCommand};
use clap::{command, Args};
use ubrn_common::{mk_dir, path_or_shim, CrateMetadata, Utf8PathBufExt as _};
use uniffi_bindgen::{cargo_metadata::CrateConfigSupplier, BindingGenerator};

#[cfg(feature = "wasm")]
use super::wasm::WasmBindingGenerator;
use super::{
    bindings::metadata::ModuleMetadata,
    react_native::ReactNativeBindingGenerator,
    switches::{AbiFlavor, SwitchArgs},
};

#[derive(Args, Debug)]
pub struct BindingsArgs {
    #[command(flatten)]
    pub(crate) source: SourceArgs,
    #[command(flatten)]
    pub(crate) output: OutputArgs,
    #[cfg(feature = "wasm")]
    #[command(flatten)]
    switches: SwitchArgs,
}

impl BindingsArgs {
    pub fn new(_switches: SwitchArgs, source: SourceArgs, output: OutputArgs) -> Self {
        Self {
            #[cfg(feature = "wasm")]
            switches: _switches,
            source,
            output,
        }
    }

    pub fn ts_dir(&self) -> &Utf8Path {
        &self.output.ts_dir
    }

    pub fn cpp_dir(&self) -> &Utf8Path {
        &self.output.cpp_dir
    }

    #[cfg(not(feature = "wasm"))]
    pub fn switches(&self) -> SwitchArgs {
        Default::default()
    }

    #[cfg(feature = "wasm")]
    pub fn switches(&self) -> SwitchArgs {
        self.switches.clone()
    }
}

#[derive(Args, Clone, Debug)]
pub struct OutputArgs {
    /// By default, bindgen will attempt to format the code with prettier and clang-format.
    #[clap(long)]
    pub(crate) no_format: bool,

    /// The directory in which to put the generated Typescript.
    #[clap(long)]
    pub(crate) ts_dir: Utf8PathBuf,

    /// The directory in which to put the generated C++.
    #[clap(long, alias = "abi-dir")]
    pub(crate) cpp_dir: Utf8PathBuf,
}

impl OutputArgs {
    pub fn new(ts_dir: &Utf8Path, cpp_dir: &Utf8Path, no_format: bool) -> Self {
        Self {
            ts_dir: ts_dir.to_owned(),
            cpp_dir: cpp_dir.to_owned(),
            no_format,
        }
    }
}

#[derive(Args, Clone, Debug, Default)]
pub struct SourceArgs {
    /// The path to a dynamic library to attempt to extract the definitions from
    /// and extend the component interface with.
    #[clap(long)]
    pub(crate) lib_file: Option<Utf8PathBuf>,

    /// Override the default crate name that is guessed from UDL file path.
    #[clap(long = "crate")]
    pub(crate) crate_name: Option<String>,

    /// The location of the uniffi.toml file
    #[clap(long)]
    pub(crate) config: Option<Utf8PathBuf>,

    /// Treat the input file as a library, extracting any Uniffi definitions from that.
    #[clap(long = "library", conflicts_with_all = ["config", "lib_file"])]
    pub(crate) library_mode: bool,

    /// A UDL file or library file
    pub(crate) source: Utf8PathBuf,
}

impl SourceArgs {
    pub fn library(file: &Utf8PathBuf) -> Self {
        Self {
            library_mode: true,
            source: file.clone(),
            ..Default::default()
        }
    }

    pub fn with_config(self, config: Option<Utf8PathBuf>) -> Self {
        Self {
            config,
            library_mode: self.library_mode,
            source: self.source,
            lib_file: self.lib_file,
            crate_name: self.crate_name,
        }
    }
}

impl BindingsArgs {
    pub fn run(&self, manifest_path: Option<&Utf8PathBuf>) -> Result<Vec<ModuleMetadata>> {
        let out = &self.output;

        mk_dir(&out.ts_dir)?;
        mk_dir(&out.cpp_dir)?;
        let ts_dir = out.ts_dir.canonicalize_utf8_or_shim()?;
        let abi_dir = out.cpp_dir.canonicalize_utf8_or_shim()?;

        let switches = self.switches();
        let abi_dir = abi_dir.clone();
        let ts_dir = ts_dir.clone();
        let cwd = Utf8PathBuf::from("Cargo.toml");
        let manifest_path = manifest_path.unwrap_or(&cwd);
        let metadata = CrateMetadata::cargo_metadata(manifest_path)?;

        match &switches.flavor {
            AbiFlavor::Jsi => self.generate_bindings(
                metadata,
                &ReactNativeBindingGenerator::new(ts_dir, abi_dir, switches),
            ),
            #[cfg(feature = "wasm")]
            AbiFlavor::Wasm => self.generate_bindings(
                metadata,
                &WasmBindingGenerator::new(ts_dir, abi_dir, switches),
            ),
        }
    }

    fn generate_bindings<Generator: BindingGenerator>(
        &self,
        metadata: Metadata,
        binding_generator: &Generator,
    ) -> std::result::Result<Vec<ModuleMetadata>, anyhow::Error> {
        let input = &self.source;
        let out = &self.output;
        let dummy_dir = Utf8PathBuf::from_str(".")?;

        let try_format_code = !out.no_format;

        let configs: Vec<ModuleMetadata> = if input.library_mode {
            let lib_path = &path_or_shim(&input.source)?;
            println!("DEBUG: Processing library file: {}", lib_path);
            println!("DEBUG: Library file exists: {}", lib_path.exists());
            println!("DEBUG: Crate name: {:?}", input.crate_name);
            println!("DEBUG: Config path: {:?}", input.config);
            
            // Use default config supplier like the working matrix-sdk xtask
            let config_supplier = CrateConfigSupplier::default();
            
            // TEST: Use KotlinBindingGenerator to see if the issue is with ReactNativeBindingGenerator
            let kotlin_generator = uniffi_bindgen::bindings::KotlinBindingGenerator;
            
            // Use full output directory path like the working xtask
            let full_output_dir = out.ts_dir.canonicalize_utf8_or_shim()?;
            
            println!("DEBUG UBRN: About to call generate_bindings with:");
            println!("DEBUG UBRN:   lib_path = {}", lib_path);
            println!("DEBUG UBRN:   crate_name = {:?}", input.crate_name);
            println!("DEBUG UBRN:   generator = KotlinBindingGenerator");
            println!("DEBUG UBRN:   config_supplier = CrateConfigSupplier::default()");
            println!("DEBUG UBRN:   config_path = {:?}", input.config.as_deref());
            println!("DEBUG UBRN:   output_dir = {}", &full_output_dir);
            println!("DEBUG UBRN:   try_format_code = {}", try_format_code);
            
            let results = uniffi_bindgen::library_mode::generate_bindings(
                lib_path,
                input.crate_name.clone(),
                &kotlin_generator,
                &config_supplier,
                input.config.as_deref(),
                &full_output_dir,
                try_format_code,
            )?;
            
            println!("DEBUG: Generated {} binding results", results.len());
            
            results
                .iter()
                .map(|s| s.into())
                .collect()
        } else {
            // For non-library mode, use config supplier from metadata
            let config_supplier = CrateConfigSupplier::from(metadata);
            uniffi_bindgen::generate_external_bindings(
                binding_generator,
                input.source.clone(),
                input.config.as_deref(),
                Some(&dummy_dir),
                input.lib_file.clone(),
                input.crate_name.as_deref(),
                try_format_code,
            )?;
            Default::default()
        };

        Ok(configs)
    }
}
