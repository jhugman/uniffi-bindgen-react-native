/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::fs;

use anyhow::Result;
use camino::{Utf8Path, Utf8PathBuf};

pub fn resolve<P: AsRef<Utf8Path>>(path: P, file_suffix: &str) -> Result<Option<Utf8PathBuf>> {
    let full_path = path.as_ref().canonicalize_utf8()?;
    resolve_from_canonical(full_path, file_suffix)
}

fn resolve_from_canonical<P: AsRef<Utf8Path>>(
    path: P,
    file_suffix: &str,
) -> Result<Option<Utf8PathBuf>> {
    let full_path = path.as_ref().join(file_suffix);
    if full_path.exists() {
        Ok(Some(full_path))
    } else if let Some(parent) = path.as_ref().parent() {
        resolve(parent, file_suffix)
    } else {
        Ok(None)
    }
}

pub fn rm_dir<P: AsRef<Utf8Path>>(dir: P) -> Result<()> {
    if dir.as_ref().exists() {
        fs::remove_dir_all(dir.as_ref())?;
    }
    Ok(())
}
