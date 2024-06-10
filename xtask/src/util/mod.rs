/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::env;

pub fn repository_root() -> Result<Utf8PathBuf> {
    let dir = env::var("CARGO_MANIFEST_DIR").context("failed to get manifest dir")?;
    Ok(Utf8Path::new(&*dir).parent().unwrap().to_path_buf())
}

pub fn build_root() -> Result<Utf8PathBuf> {
    let dir = repository_root()?;
    Ok(dir.join("build"))
}

pub fn cpp_modules() -> Result<Utf8PathBuf> {
    let dir = repository_root()?;
    Ok(dir.join("cpp_modules"))
}
