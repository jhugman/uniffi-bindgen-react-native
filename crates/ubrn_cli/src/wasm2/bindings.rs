/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8PathBuf;

use ubrn_bindgen::{BindingsArgs, OutputArgs, SourceArgs, SwitchArgs};

use crate::config::ProjectConfig;

pub(crate) fn bindings(
    project: &ProjectConfig,
    switches: SwitchArgs,
    lib_file: &Utf8PathBuf,
) -> Result<BindingsArgs> {
    let root = project.project_root();
    let config = project.bindings.uniffi_toml_path(root);
    if let Some(ref file) = config {
        if !file.exists() {
            anyhow::bail!("uniffi.toml file {:?} does not exist. Either delete the uniffiToml property or supply a file", file)
        }
    }
    let source = SourceArgs::library(lib_file).with_config(config);

    let ts_dir = project.wasm2_bindings_ts_path(root);
    // Wasm2 generates no C++ and no Rust shim, but `OutputArgs` requires a
    // native-output directory and creates it. Point it at the TypeScript dir
    // so we don't leave an empty directory behind in the user's project.
    Ok(BindingsArgs::new(
        switches,
        source,
        OutputArgs::new(&ts_dir, &ts_dir, false),
    ))
}
