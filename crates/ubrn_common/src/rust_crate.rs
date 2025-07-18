/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{path::PathBuf, process::Command};

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{Metadata, MetadataCommand, TargetKind};

use crate::{path_or_shim, run_cmd_quietly};

#[derive(Debug, Clone)]
pub struct CrateMetadata {
    pub(crate) manifest_path: Utf8PathBuf,
    pub(crate) crate_dir: Utf8PathBuf,
    pub(crate) target_dir: Utf8PathBuf,
    pub(crate) package_name: String,
    pub(crate) library_name: String,
}

impl CrateMetadata {
    pub fn profile(profile: Option<&str>, release: bool) -> &str {
        profile.unwrap_or(if release { "release" } else { "debug" })
    }

    pub fn library_path(&self, target: Option<&str>, profile: &str, use_shared_library: Option<bool>) -> Utf8PathBuf {
        let library_name = self.library_file(target, use_shared_library);
        match target {
            Some(t) => self.target_dir.join(t).join(profile).join(library_name),
            None => self.target_dir.join(profile).join(library_name),
        }
    }

    pub fn library_path_exists(&self, path: &Utf8Path) -> Result<()> {
        if !path.exists() {
            anyhow::bail!("Library doesn't exist. This may be because `staticlib` is not in the `crate-type` list in the [lib] entry of Cargo.toml: {}", self.manifest_path());
        }
        Ok(())
    }

    pub fn library_file(&self, target: Option<&str>, use_shared_library: Option<bool>) -> String {
        let ext = so_extension(target, use_shared_library);
        if ext == "wasm" {
            format!("{}.{ext}", &self.library_name)
        } else {
            format!("lib{}.{ext}", &self.library_name)
        }
    }

    pub fn target_dir(&self) -> &Utf8Path {
        &self.target_dir
    }

    pub fn crate_dir(&self) -> &Utf8Path {
        &self.crate_dir
    }

    pub fn package_name(&self) -> &str {
        &self.package_name
    }

    pub fn library_name(&self) -> &str {
        &self.library_name
    }

    pub fn project_root(&self) -> &Utf8Path {
        self.target_dir
            .parent()
            .expect("Project root is the target_dir parent")
    }

    pub fn manifest_path(&self) -> &Utf8Path {
        &self.manifest_path
    }

    pub fn cargo_clean(&self) -> Result<()> {
        let mut cmd = Command::new("cargo");
        run_cmd_quietly(cmd.arg("clean").current_dir(&self.crate_dir))?;
        Ok(())
    }

    pub fn cargo_metadata_cwd() -> Result<Metadata> {
        // Run `cargo metadata`
        Ok(MetadataCommand::new().exec()?)
    }

    pub fn cargo_metadata(manifest_path: impl Into<PathBuf>) -> Result<Metadata> {
        // Run `cargo metadata`
        Ok(MetadataCommand::new().manifest_path(manifest_path).exec()?)
    }
}

impl CrateMetadata {
    pub fn from_metadata(metadata: &Metadata, manifest_path: &Utf8Path) -> Result<Self> {
        if !manifest_path.exists() {
            anyhow::bail!("Manifest not found at {manifest_path}");
        }
        let manifest_path = manifest_path.canonicalize_utf8()?;
        let crate_dir = manifest_path
            .parent()
            .expect("A valid parent for the crate manifest")
            .into();
        let library_name = guess_library_name(metadata, &manifest_path);
        let package_name = find_package_name(metadata, &manifest_path)
            .expect("A [package] `name` was not found in the manifest");
        let target_dir = metadata.target_directory.clone();

        Ok(Self {
            manifest_path,
            package_name,
            library_name,
            target_dir,
            crate_dir,
        })
    }
}

pub fn so_extension<'a>(target: Option<&str>, use_shared_library: Option<bool>) -> &'a str {
    match target {
        Some(t) => so_extension_from_target(t, use_shared_library),
        _ => so_extension_from_cfg(),
    }
}

fn so_extension_from_target<'a>(target: &str, use_shared_library: Option<bool>) -> &'a str {
    if target.contains("windows") {
        "dll"
    } else if target.contains("darwin") {
        "dylib"
    } else if target.contains("ios") {
        "a"
    } else if target.contains("android") && use_shared_library.expect("By default we use static libs on android") {
        "so"
    } else if target.contains("android") {
        "a"
    } else if target.contains("wasm") {
        "wasm"
    } else {
        unimplemented!("Building targeting only on android and ios supported right now")
    }
}

fn so_extension_from_cfg<'a>() -> &'a str {
    if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "linux") {
        "so"
    } else {
        unimplemented!("Building only on windows, macos and linux supported right now")
    }
}

impl TryFrom<Utf8PathBuf> for CrateMetadata {
    type Error = anyhow::Error;

    fn try_from(manifest_path: Utf8PathBuf) -> Result<Self> {
        let manifest_path = path_or_shim(&manifest_path)?;
        if !manifest_path.exists() {
            anyhow::bail!("Crate manifest doesn't exist");
        }
        let manifest_path = manifest_path.canonicalize_utf8()?;
        let manifest_path = if !manifest_path.ends_with("Cargo.toml") {
            if !manifest_path.is_dir() {
                anyhow::bail!("Crate should either be a path to a Cargo.toml or a directory containing a Cargo.toml file");
            }
            manifest_path.join("Cargo.toml")
        } else {
            manifest_path
        };
        let metadata = Self::cargo_metadata(&manifest_path)?;
        Self::from_metadata(&metadata, &manifest_path)
    }
}

fn guess_library_name(metadata: &Metadata, manifest_path: &Utf8Path) -> String {
    find_library_name(metadata, manifest_path)
        .unwrap_or_else(|| {
            find_package_name(metadata, manifest_path).expect(
                "Neither a [[package]] `name` or a [[lib]] `name` were found in the manifest",
            )
        })
        .replace('-', "_")
}

fn find_library_name(metadata: &Metadata, manifest_path: &Utf8Path) -> Option<String> {
    // Get the library name
    let lib = TargetKind::Lib;
    metadata
        .packages
        .iter()
        .find(|package| package.manifest_path == *manifest_path)
        .and_then(|package| {
            package
                .targets
                .iter()
                .find(|target| target.kind.contains(&lib))
        })
        .map(|target| target.name.clone())
}

fn find_package_name(metadata: &Metadata, manifest_path: &Utf8Path) -> Option<String> {
    metadata
        .packages
        .iter()
        .find(|package| package.manifest_path == *manifest_path)
        .map(|package| package.name.clone())
}
