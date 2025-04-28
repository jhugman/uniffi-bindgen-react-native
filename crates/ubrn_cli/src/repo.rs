/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;

use crate::{config::ProjectConfig, source::GitRepoArgs, AsConfig};

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
