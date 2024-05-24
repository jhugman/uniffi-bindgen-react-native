/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::fs;

use anyhow::{bail, Result};
use camino::{Utf8Path, Utf8PathBuf};

/// Finds a file in the given directory.
///
/// If None exists, then search in the parent directory, recursively until it is found.
/// If None is found, then return None.
pub fn resolve<P: AsRef<Utf8Path>>(directory: P, file_suffix: &str) -> Result<Option<Utf8PathBuf>> {
    let full_path = directory.as_ref().canonicalize_utf8()?;
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
        resolve_from_canonical(parent, file_suffix)
    } else {
        Ok(None)
    }
}

/// Search the directory for a file with the given filename.
///
/// If none exists, return None.
pub fn find<P: AsRef<Utf8Path>>(directory: P, filename: &str) -> Option<Utf8PathBuf> {
    let path = glob::glob(&format!("{base}/**/{filename}", base = directory.as_ref()))
        .unwrap()
        .find_map(Result::ok)?;
    let path: Utf8PathBuf = path.try_into().unwrap_or_else(|_| panic!("not a utf path"));
    Some(path)
}

pub fn file_paths(pattern: &str) -> Result<Vec<std::ffi::OsString>, anyhow::Error> {
    let files = glob::glob(pattern)?;
    let files: Vec<_> = files
        .into_iter()
        .map(|pb| {
            let file = pb.expect("is valid PathBuf");
            file.into_os_string()
        })
        .collect();
    Ok(files)
}

pub fn pwd() -> Result<Utf8PathBuf> {
    let path = std::env::current_dir()?;
    Ok(Utf8PathBuf::try_from(path)?)
}

pub fn rm_dir<P: AsRef<Utf8Path>>(dir: P) -> Result<()> {
    if dir.as_ref().exists() {
        fs::remove_dir_all(dir.as_ref())?;
    }
    Ok(())
}

pub fn mk_dir<P: AsRef<Utf8Path>>(dir: P) -> Result<()> {
    let dir = pwd()?.join(dir);
    if dir.exists() {
        if dir.is_dir() {
            Ok(())
        } else {
            bail!("{dir} is supposed to be a directory but is not")
        }
    } else {
        fs::create_dir_all(dir)?;
        Ok(())
    }
}
