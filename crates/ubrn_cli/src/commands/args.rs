/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use camino::Utf8PathBuf;
use clap::{self, Args};

use crate::{workspace, ProjectConfig};

#[derive(Args, Clone, Debug)]
pub(crate) struct ConfigArgs {
    /// The configuration file for this project
    #[clap(long)]
    config: Option<Utf8PathBuf>,
}

impl ConfigArgs {
    pub(crate) fn new(config: Option<Utf8PathBuf>) -> Self {
        Self { config }
    }
}

fn default_config_path() -> Utf8PathBuf {
    workspace::ubrn_config_yaml()
        .expect("Can't find a ubrn.config.yaml file: specify one with a `--config` argument")
}

impl Default for ConfigArgs {
    fn default() -> Self {
        let config = Some(default_config_path());
        Self { config }
    }
}

impl TryFrom<ConfigArgs> for ProjectConfig {
    type Error = anyhow::Error;

    fn try_from(value: ConfigArgs) -> Result<Self, Self::Error> {
        let path = value.config.unwrap_or_else(default_config_path);
        Self::try_from(path)
    }
}
