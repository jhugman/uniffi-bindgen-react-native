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

use crate::{config::ProjectConfig, AsConfig};

use super::ConfigArgs;

#[derive(Debug, Args)]
pub(crate) struct CheckoutArgs {
    #[clap(long, conflicts_with_all = ["repo"])]
    config: Option<Utf8PathBuf>,

    #[clap(flatten)]
    repo: Option<RepoArgs>,
}

#[derive(Args, Clone, Debug)]
struct RepoArgs {
    /// The repository where to get the crate
    repo: Option<String>,
    /// The branch or tag which to checkout
    #[clap(long, default_value = "main")]
    branch: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct GitRepoArgs {
    /// The repository where to get the crate
    repo: String,
    /// The branch or tag which to checkout
    #[serde(alias = "rev", alias = "ref", default = "GitRepoArgs::default_branch")]
    branch: String,
}

impl GitRepoArgs {
    fn default_branch() -> String {
        "main".into()
    }

    pub(crate) fn directory(&self, project_root: &Utf8Path) -> Result<Utf8PathBuf> {
        // Use Utf8Path for URL operations is a little bit hacky,
        // but as we only need URL for this operation, we can avoid
        // dragging in another dependency.
        let url_path = Utf8Path::new(self.repo.as_str());
        let repo_name = url_path.file_name().unwrap();
        let repo_name = repo_name.strip_suffix(".git").unwrap_or(repo_name);
        Ok(project_root.join("rust_modules").join(repo_name))
    }

    pub(crate) fn checkout(&self, project_root: &Utf8Path) -> Result<()> {
        // git clone --depth 1 if directory doesn't already exist
        if !self.directory(project_root)?.exists() {
            let mut cmd = Command::new("git");
            cmd.arg("clone")
                .arg(&self.repo)
                .arg(self.directory(project_root)?)
                .arg("--depth")
                .arg("1");
            run_cmd(&mut cmd)?;
        }

        // git remote set-branches origin '*' to start tracking all branches and
        // enable checking out branch names
        let mut cmd = Command::new("git");
        cmd.current_dir(self.directory(project_root)?)
            .arg("remote")
            .arg("set-branches")
            .arg("origin")
            .arg("*");
        run_cmd(&mut cmd)?;

        // git ls-remote --tags origin $branch
        let output = Command::new("git")
            .current_dir(self.directory(project_root)?)
            .arg("ls-remote")
            .arg("--tags")
            .arg("origin")
            .arg(&self.branch)
            .output()?;
        let output = String::from_utf8(output.stdout)?;

        // Find $branch in the output and resolve the SHA or fall back to $branch. We
        // deliberately don't resolve branch names to their SHAs because checking out
        // with the SHA would cause a detached HEAD state.
        let tag_ref = format!("refs/tags/{}", &self.branch);
        let sha = output
            .lines()
            .find(|line| line.ends_with(&tag_ref))
            .map(|line| {
                line.split_whitespace()
                    .next()
                    .expect("Git lines have sha then space")
            })
            .unwrap_or(&self.branch);

        // git fetch --depth 1 origin $sha
        let mut cmd = Command::new("git");
        cmd.current_dir(self.directory(project_root)?)
            .arg("fetch")
            .arg("--depth")
            .arg("1")
            .arg("origin")
            .arg(sha);
        run_cmd(&mut cmd)?;

        // git checkout $sha
        let mut cmd = Command::new("git");
        cmd.current_dir(self.directory(project_root)?)
            .arg("checkout")
            .arg(sha);
        run_cmd(&mut cmd)
    }
}

impl TryFrom<ProjectConfig> for GitRepoArgs {
    type Error = anyhow::Error;

    fn try_from(value: ProjectConfig) -> Result<Self> {
        value.crate_.src.try_into()
    }
}

impl AsConfig<GitRepoArgs> for CheckoutArgs {
    fn config_file(&self) -> ConfigArgs {
        ConfigArgs::new(self.config.clone())
    }

    fn get(&self) -> Option<GitRepoArgs> {
        let args = self.repo.clone()?;
        let branch = args.branch;
        let repo = args.repo?;
        Some(GitRepoArgs { repo, branch })
    }
}
