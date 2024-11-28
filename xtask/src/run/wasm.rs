/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use super::{NodeJs, RunCmd};
use crate::bootstrap::{Bootstrap, YarnCmd};
use anyhow::{Ok, Result};

pub(crate) struct Wasm;

impl Wasm {
    pub(crate) fn run(&self, cmd: &RunCmd) -> Result<()> {
        YarnCmd.ensure_ready()?;
        let js_file = cmd.js_file.file.canonicalize_utf8()?;
        NodeJs.tsx(&js_file)?;
        Ok(())
    }
}
