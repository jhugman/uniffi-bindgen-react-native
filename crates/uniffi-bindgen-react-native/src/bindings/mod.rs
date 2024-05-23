/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

pub(crate) mod react_native;

use std::str::FromStr;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{command, Args};
use uniffi_common::mk_dir;

use self::react_native::ReactNativeBindingGenerator;

#[derive(Args, Debug)]
pub(crate) struct BindingsArgs {
    #[command(flatten)]
    source: SourceArgs,
    #[command(flatten)]
    output: OutputArgs,
}

#[derive(Args, Clone, Debug)]
pub(crate) struct OutputArgs {
    /// By default, bindgen will attempt to format the code with prettier and clang-format.
    #[clap(long)]
    no_format: bool,

    /// The directory in which to put the generated Typescript.
    #[clap(long)]
    ts_dir: Utf8PathBuf,

    /// The directory in which to put the generated C++.
    #[clap(long)]
    cpp_dir: Utf8PathBuf,
}

#[derive(Args, Clone, Debug)]
pub(crate) struct SourceArgs {
    /// The path to a dynamic library to attempt to extract the definitions from
    /// and extend the component interface with.
    #[clap(long)]
    lib_file: Option<Utf8PathBuf>,

    /// Override the default crate name that is guessed from UDL file path.
    ///
    /// In library mode, this
    #[clap(long = "crate")]
    crate_name: Option<String>,

    /// The location of the uniffi.toml file
    #[clap(long)]
    config: Option<Utf8PathBuf>,

    /// Treat the input file as a library, extracting any Uniffi definitions from that.
    #[clap(long = "library", conflicts_with_all = ["config", "lib_file"])]
    library_mode: bool,

    /// A UDL file or library file
    source: Utf8PathBuf,
}

impl BindingsArgs {
    pub(crate) fn run(&self) -> Result<()> {
        let input = &self.source;
        let out = &self.output;

        mk_dir(&out.ts_dir)?;
        mk_dir(&out.cpp_dir)?;

        let generator = ReactNativeBindingGenerator::new(out.clone());
        let dummy_dir = Utf8PathBuf::from_str(".")?;

        let try_format_code = !out.no_format;

        if input.library_mode {
            uniffi_bindgen::library_mode::generate_external_bindings(
                &generator,
                &input.source,
                input.crate_name.clone(),
                input.config.as_deref(),
                &dummy_dir,
                try_format_code,
            )
            .unwrap();
        } else {
            uniffi_bindgen::generate_external_bindings(
                &generator,
                input.source.clone(),
                input.config.as_deref(),
                Some(&dummy_dir),
                input.lib_file.clone(),
                input.crate_name.as_deref(),
                try_format_code,
            )
            .unwrap();
        }

        if try_format_code {
            let _ = generator.format_code();
        }

        Ok(())
    }
}
