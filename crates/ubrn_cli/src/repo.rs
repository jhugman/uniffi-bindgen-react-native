/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command;

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use serde::Deserialize;
use ubrn_common::run_cmd;

use crate::{config::ProjectConfig, workspace, AsConfig};

#[derive(Args, Clone, Debug, Deserialize)]
pub(crate) struct GitRepoArgs {
    /// The repository where to get the crate
    pub(crate) repo: String,
    /// The branch or tag which to checkout
    #[clap(long, default_value = "main")]
    #[serde(default = "GitRepoArgs::default_branch")]
    pub(crate) branch: String,
}

impl GitRepoArgs {
    fn default_branch() -> String {
        "main".into()
    }
}

#[derive(Debug, Args)]
pub(crate) struct CheckoutArgs {
    #[clap(long, conflicts_with_all = ["repo"])]
    config: Option<Utf8PathBuf>,

    #[clap(flatten)]
    repo: Option<GitRepoArgs>,
}

impl TryFrom<ProjectConfig> for GitRepoArgs {
    type Error = anyhow::Error;

    fn try_from(value: ProjectConfig) -> Result<Self> {
        value.crate_.src.try_into()
    }
}

impl AsConfig<GitRepoArgs> for CheckoutArgs {
    fn config_file(&self) -> Option<Utf8PathBuf> {
        self.config.clone()
    }

    fn get(&self) -> Option<GitRepoArgs> {
        self.repo.clone()
    }
}

impl GitRepoArgs {
    pub(crate) fn directory(&self) -> Result<Utf8PathBuf> {
        // Use Utf8Path for URL operations is a little bit hacky,
        // but as we only need URL for this operation, we can avoid
        // dragging in another dependency.
        let url_path = Utf8Path::new(self.repo.as_str());
        let repo_name = url_path.file_name().unwrap();
        let repo_name = repo_name.strip_suffix(".git").unwrap_or(repo_name);
        let root = workspace::project_root()?;
        Ok(root.join("rust_modules").join(repo_name))
    }

    pub(crate) fn checkout(&self) -> Result<()> {
        let mut cmd = Command::new("git");
        cmd.arg("clone")
            .arg(&self.repo)
            .arg(self.directory()?)
            .arg("--single-branch")
            .arg("--branch")
            .arg(&self.branch);
        run_cmd(&mut cmd)
    }
}
