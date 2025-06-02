/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::fs;

use anyhow::{bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

use crate::testing::is_recording_enabled;

/// Finds a file in the given directory.
///
/// If None exists, then search in the parent directory, recursively until it is found.
/// If None is found, then return None.
pub fn resolve<P: AsRef<Utf8Path>>(directory: P, file_suffix: &str) -> Result<Option<Utf8PathBuf>> {
    let full_path = directory.as_ref().canonicalize_utf8_or_shim()?;
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

pub fn cd(dir: &Utf8Path) -> Result<()> {
    let dir = path_or_shim(dir)?;
    std::env::set_current_dir(dir)?;
    Ok(())
}

pub fn rm_dir<P: AsRef<Utf8Path>>(dir: P) -> Result<()> {
    if dir.as_ref().exists() {
        fs::remove_dir_all(dir.as_ref())?;
    }
    Ok(())
}

pub fn rm_file<P: AsRef<Utf8Path>>(file: P) -> Result<()> {
    if file.as_ref().exists() {
        fs::remove_file(file.as_ref())?;
    }
    Ok(())
}

pub fn cp_file<P: AsRef<Utf8Path>, D: AsRef<Utf8Path>>(src: P, dst: D) -> Result<()> {
    if is_recording_enabled() {
        return Ok(());
    }
    let src = src.as_ref();
    let dst = dst.as_ref();
    if !src.exists() {
        bail!("File {src} does not exist");
    }
    fs::copy(src, dst).with_context(|| format!("Failed to copy {src} to {dst}"))?;
    Ok(())
}

pub fn mk_dir<P: AsRef<Utf8Path>>(dir: P) -> Result<()> {
    if is_recording_enabled() {
        return Ok(());
    }
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

pub fn mv_files(extension: &str, source: &Utf8Path, destination: &Utf8Path) -> Result<()> {
    if is_recording_enabled() {
        return Ok(());
    }
    for entry in source.read_dir_utf8()? {
        let entry = entry?;
        let path = entry.path();
        if !entry.file_type()?.is_file() || path.extension() != Some(extension) {
            continue;
        }
        let file_name = path.file_name().expect("Could not get file name from path");
        fs::rename(path, destination.join(file_name))?
    }
    Ok(())
}

pub fn read_from_file<P, T>(file: P) -> Result<T>
where
    P: AsRef<Utf8Path>,
    for<'a> T: Deserialize<'a>,
{
    let file = file.as_ref();
    let s = read_to_string(file)?;
    Ok(if is_yaml(file) {
        serde_yaml::from_str(&s)
            .with_context(|| format!("Failed to read {file:?} as valid YAML"))?
    } else if is_toml(file) {
        toml::from_str(&s).with_context(|| format!("Failed to read {file:?} as valid TOML"))?
    } else {
        serde_json::from_str(&s)
            .with_context(|| format!("Failed to read {file:?} as valid YAML or JSON"))?
    })
}

pub fn read_to_string<P>(file: P) -> Result<String>
where
    P: AsRef<Utf8Path>,
{
    let file = file.as_ref();

    // If we're recording and a file shim exists, use it instead of the actual file
    if is_recording_enabled() {
        if let Some(shim_source) = crate::testing::get_shimmed_path(file) {
            match shim_source {
                crate::testing::ShimSource::FilePath(path) => {
                    let replacement_path = Utf8Path::new(&path);
                    if replacement_path.exists() {
                        return fs::read_to_string(replacement_path).with_context(|| {
                            format!("Failed to read from shimmed file {replacement_path:?}")
                        });
                    } else {
                        anyhow::bail!("Shimmed file {replacement_path} does not exist");
                    }
                }
                crate::testing::ShimSource::StringContent(content) => {
                    // Return string content directly
                    return Ok(content);
                }
            }
        }
    }
    if !file.exists() {
        anyhow::bail!("File {file} does not exist");
    }

    fs::read_to_string(file).with_context(|| format!("Failed to read from {file:?}"))
}

fn is_yaml<P>(file: P) -> bool
where
    P: AsRef<Utf8Path>,
{
    let ext = file.as_ref().extension().unwrap_or_default();
    ext == "yaml" || ext == "yml"
}

fn is_toml<P>(file: P) -> bool
where
    P: AsRef<Utf8Path>,
{
    let ext = file.as_ref().extension().unwrap_or_default();
    ext == "toml"
}

pub fn write_file<P: AsRef<Utf8Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    let path = path.as_ref();

    // If we're in recording mode, don't actually write the file
    if is_recording_enabled() {
        crate::testing::record_file(path, &contents);
        return Ok(());
    }

    fs::write(path, contents).with_context(|| format!("Failed to write to {path}"))?;
    Ok(())
}

pub fn path_or_shim(file_path: &Utf8Path) -> Result<Utf8PathBuf> {
    use crate::testing::{get_shimmed_path, ShimSource};
    Ok(if !is_recording_enabled() || file_path.exists() {
        file_path.to_path_buf()
    } else {
        match get_shimmed_path(file_path) {
            Some(ShimSource::FilePath(path)) => Utf8PathBuf::from(path),
            Some(_) => anyhow::bail!("File at {file_path} is shimmed, but the path is not"),
            None => anyhow::bail!("File at {file_path} doesn't exist and isn't shimmed"),
        }
    })
}

#[extend::ext]
pub impl Utf8Path {
    fn canonicalize_utf8_or_shim(&self) -> Result<Utf8PathBuf> {
        Ok(if is_recording_enabled() {
            self.to_path_buf()
        } else {
            self.canonicalize_utf8()?
        })
    }
}

#[extend::ext]
pub impl Utf8PathBuf {
    fn canonicalize_utf8_or_shim(&self) -> Result<Utf8PathBuf> {
        self.as_path().canonicalize_utf8_or_shim()
    }
}
