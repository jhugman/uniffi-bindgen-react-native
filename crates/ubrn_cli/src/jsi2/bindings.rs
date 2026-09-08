/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8PathBuf;
use ubrn_bindgen::{
    ffi_module_player_lib_resolution::LibResolution, BindingsArgs, OutputArgs, SourceArgs,
    SwitchArgs,
};

use crate::config::ProjectConfig;

/// Bindings that open the library by its cdylib name: every module of the
/// library names the same file, so its uniffi statics are opened once.
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
    let lib_name = project.crate_.metadata()?.library_name().to_string();
    let ts_dir = project.jsi2_bindings_ts_path(root);
    // No native output, but OutputArgs creates its directory; point it at the
    // TypeScript one rather than leave an empty directory behind.
    Ok(BindingsArgs::new(
        switches,
        SourceArgs::library(lib_file).with_config(config),
        OutputArgs::new(&ts_dir, &ts_dir, false),
    )
    .with_lib_resolution(LibResolution::Name(lib_name)))
}
