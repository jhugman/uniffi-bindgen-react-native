/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::rc::Rc;

use anyhow::{anyhow, Result};
use camino::{Utf8Path, Utf8PathBuf};

use ubrn_bindgen::{generate_entrypoint, AbiFlavor, SwitchArgs};

use crate::codegen::{RenderedFile, TemplateConfig};
use crate::templated_file;

pub(crate) fn get_files(config: Rc<TemplateConfig>) -> Vec<Rc<dyn RenderedFile>> {
    vec![
        WasmCargoToml::rc_new(config.clone()),
        WasmLibRs::rc_new(config.clone()),
        IndexWebTs::rc_new(config.clone()),
    ]
}

templated_file!(WasmCargoToml, "Cargo.toml");
impl RenderedFile for WasmCargoToml {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.config.project.wasm.manifest_path(project_root)
    }
    fn transform_str(&self, project_root: &Utf8Path, contents: String) -> Result<String> {
        Ok(
            if let Some(patch_file) = &self.config.project.wasm.manifest_patch_file {
                use toml::to_string;
                let path = project_root.join(patch_file);
                let patch: toml::Value = ubrn_common::read_from_file(&path)?;
                let rendered: toml::Value = toml::from_str(&contents)?;
                let merged: toml::Value = serde_toml_merge::merge(rendered, patch)
                    .map_err(|e| anyhow!("TOML merge error: {e}"))?;
                to_string(&merged)?
            } else {
                contents
            },
        )
    }
}
impl WasmCargoToml {
    fn runtime_version(&self) -> String {
        self.config.project.wasm.runtime_version()
    }
}

templated_file!(WasmLibRs, "lib.rs");
impl RenderedFile for WasmLibRs {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let filename = "src/lib.rs";
        self.config
            .project
            .wasm
            .crate_dir(project_root)
            .join(filename)
    }
}

impl WasmLibRs {
    fn entrypoint(&self) -> String {
        let switches = SwitchArgs {
            flavor: AbiFlavor::Wasm,
        };
        generate_entrypoint(&switches, &self.config.rust_crate, &self.config.modules).unwrap()
    }
}

templated_file!(IndexWebTs, "index.web.ts");
impl RenderedFile for IndexWebTs {
    fn path(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        self.config.project.wasm.entrypoint(project_root)
    }
}
