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
    let ts_dir = project.bindings.ts_path(root);
    let cpp_dir = project.bindings.cpp_path(root);

    let config = project.bindings.uniffi_toml_path(project.project_root());
    if let Some(ref file) = config {
        if !file.exists() {
            anyhow::bail!("uniffi.toml file {:?} does not exist. Either delete the uniffiToml property or supply a file", file)
        }
    }

    Ok(BindingsArgs::new(
        switches,
        SourceArgs::library(lib_file).with_config(config),
        OutputArgs::new(&ts_dir, &cpp_dir, false),
    ))
}
