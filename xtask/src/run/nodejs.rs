/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::process::Command;

use anyhow::{Ok, Result};
use camino::Utf8Path;

use crate::bootstrap::YarnCmd;

pub(crate) struct NodeJs;

impl NodeJs {
    pub(crate) fn tsx(&self, file: &Utf8Path) -> Result<()> {
        let node_modules = YarnCmd::node_modules()?;
        let Some(tsx) = ubrn_common::find(node_modules, ".bin/tsx") else {
            unreachable!("Can't find tsx; this is likely a change in how tsx is packaged");
        };
        let mut cmd = Command::new(tsx);
        cmd.arg("--experimental-wasm-modules").arg(file);
        ubrn_common::run_cmd(&mut cmd)?;
        Ok(())
    }
}
