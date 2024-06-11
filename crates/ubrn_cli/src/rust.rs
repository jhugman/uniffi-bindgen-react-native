/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use serde::Deserialize;

use anyhow::{Error, Result};
use camino::Utf8PathBuf;
use ubrn_common::CrateMetadata;

use crate::{repo::GitRepoArgs, workspace};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CrateConfig {
    #[serde(default = "CrateConfig::default_manifest_path")]
    pub(crate) manifest_path: String,
    #[serde(flatten)]
    pub(crate) src: RustSource,
}

impl CrateConfig {
    fn default_manifest_path() -> String {
        "Cargo.toml".to_string()
    }
}

impl CrateConfig {
    pub(crate) fn directory(&self) -> Result<Utf8PathBuf> {
        self.src.directory()
    }

    pub(crate) fn manifest_path(&self) -> Result<Utf8PathBuf> {
        Ok(self.directory()?.join(&self.manifest_path))
    }

    pub(crate) fn metadata(&self) -> Result<CrateMetadata> {
        self.manifest_path()?.try_into()
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum RustSource {
    OnDisk(OnDiskArgs),
    GitRepo(GitRepoArgs),
}

#[derive(Debug, Deserialize)]
pub(crate) struct OnDiskArgs {
    #[serde(alias = "rust")]
    pub(crate) src: String,
}

impl RustSource {
    pub(crate) fn directory(&self) -> Result<Utf8PathBuf> {
        Ok(match self {
            Self::OnDisk(OnDiskArgs { src }) => workspace::project_root()?.join(src),
            Self::GitRepo(c) => c.directory()?,
        })
    }
}

impl TryFrom<RustSource> for GitRepoArgs {
    type Error = Error;

    fn try_from(value: RustSource) -> Result<Self> {
        match value {
            RustSource::GitRepo(args) => Ok(args),
            _ => anyhow::bail!("Nothing to do"),
        }
    }
}
