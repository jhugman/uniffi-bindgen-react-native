/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8PathBuf;

use crate::config::PackageJson;

fn find_file_in_parents(file_suffix: &str) -> Result<Utf8PathBuf> {
    let pwd = ubrn_common::pwd()?;
    Ok(match ubrn_common::resolve(pwd, file_suffix)? {
        Some(file) => file,
        _ => anyhow::bail!("{file_suffix} can't be found in current directory"),
    })
}

pub(crate) fn package_json() -> PackageJson {
    let file = find_file_in_parents("package.json").expect("Cannot find package.json");
    ubrn_common::read_from_file(file).expect("Cannot load package.json")
}

pub(crate) fn project_root() -> Result<Utf8PathBuf> {
    let package_json = find_file_in_parents("package.json")?;
    let dir = package_json.parent().expect("Must be a directory");
    Ok(dir.into())
}

pub(crate) fn ubrn_config_yaml() -> Result<Utf8PathBuf> {
    find_file_in_parents("ubrn.config.yaml")
}
