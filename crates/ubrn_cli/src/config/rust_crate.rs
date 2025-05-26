/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::{convert::TryFrom, path::Path};

use anyhow::{Error, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Deserializer};

use ubrn_common::{path_or_shim, CrateMetadata};

use crate::{commands::checkout::GitRepoArgs, workspace};

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum RustSource {
    OnDisk(OnDiskArgs),
    GitRepo(GitRepoArgs),
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct OnDiskArgs {
    #[serde(alias = "rust", alias = "directory")]
    pub(crate) src: String,
}

impl RustSource {
    pub(crate) fn directory(&self, project_root: &Utf8Path) -> Result<Utf8PathBuf> {
        Ok(match self {
            Self::OnDisk(OnDiskArgs { src }) => project_root.join(src),
            Self::GitRepo(c) => c.directory(project_root)?,
        })
    }
}

impl TryFrom<RustSource> for GitRepoArgs {
    type Error = Error;

    fn try_from(value: RustSource) -> Result<Self> {
        match value {
            RustSource::GitRepo(args) => Ok(args),
            _ => anyhow::bail!("Not a Git repository source"),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrateConfig {
    #[serde(default = "CrateConfig::default_project_root", skip)]
    pub(crate) project_root: Utf8PathBuf,

    #[serde(default = "CrateConfig::default_manifest_path")]
    #[serde(deserialize_with = "CrateConfig::validate_manifest_path")]
    pub(crate) manifest_path: String,
    #[serde(flatten)]
    pub(crate) src: RustSource,
}

impl CrateConfig {
    fn default_project_root() -> Utf8PathBuf {
        workspace::project_root().expect("Expected project root with a package.json")
    }

    fn default_manifest_path() -> String {
        "Cargo.toml".to_string()
    }

    pub(crate) fn validate_manifest_path<'de, D>(deserializer: D) -> Result<String, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path_str = String::deserialize(deserializer)?;

        // Validate the path ends with Cargo.toml
        if !path_str.ends_with("Cargo.toml") {
            return Err(serde::de::Error::custom(format!(
                "Manifest path must end with 'Cargo.toml': {}",
                path_str
            )));
        }

        // Validate that the path doesn't contain invalid characters
        if Path::new(&path_str).file_name().is_none() {
            return Err(serde::de::Error::custom(format!(
                "Invalid manifest path: {}",
                path_str
            )));
        }

        Ok(path_str)
    }
}

impl CrateConfig {
    pub(crate) fn directory(&self) -> Result<Utf8PathBuf> {
        self.src.directory(&self.project_root)
    }

    pub(crate) fn manifest_path(&self) -> Result<Utf8PathBuf> {
        let manifest_path = path_or_shim(&self.directory()?.join(&self.manifest_path))?;
        Ok(manifest_path)
    }

    pub(crate) fn crate_dir(&self) -> Result<Utf8PathBuf> {
        let manifest = self.manifest_path()?;
        let dir = manifest.parent().unwrap();
        Ok(dir.into())
    }

    #[allow(dead_code)]
    pub(crate) fn crate_dir_relative(&self, project_root: &Utf8Path) -> Utf8PathBuf {
        let manifest = self
            .src
            .directory(project_root)
            .unwrap()
            .join(&self.manifest_path);
        let crate_dir = manifest.parent().expect("Expect a parent here");
        crate_dir.into()
    }

    pub(crate) fn metadata(&self) -> Result<CrateMetadata> {
        self.manifest_path()?.try_into()
    }
}

impl TryFrom<CrateMetadata> for CrateConfig {
    type Error = Error;

    fn try_from(value: CrateMetadata) -> Result<Self> {
        let project_root = value.project_root();
        let manifest_path = value.manifest_path();
        let diff = pathdiff::diff_utf8_paths(manifest_path, project_root)
            .expect("Manifest path should be relative to the workspace root");
        Ok(Self {
            project_root: value.project_root().to_path_buf(),
            manifest_path: diff.into_string(),
            src: RustSource::OnDisk(OnDiskArgs {
                src: ".".to_string(),
            }),
        })
    }
}
