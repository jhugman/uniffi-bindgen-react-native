/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use ubrn_common::{rm_dir, run_cmd};

use crate::util::repository_root;

use super::Bootstrap;

#[derive(Debug, Args)]
pub(crate) struct YarnCmd;

impl YarnCmd {
    pub(crate) fn node_modules() -> Result<Utf8PathBuf> {
        let root = repository_root()?;
        Ok(root.join("node_modules"))
    }
}

impl Bootstrap for YarnCmd {
    fn marker() -> Result<Utf8PathBuf> {
        Self::node_modules()
    }

    fn clean() -> Result<()> {
        rm_dir(Self::node_modules()?)
    }

    fn prepare(&self) -> Result<()> {
        let mut cmd = Command::new("yarn");
        run_cmd(
            cmd.current_dir(repository_root()?)
                .arg("--frozen-lockfile")
                .arg("--emoji")
                .arg("true"),
        )
    }
}
