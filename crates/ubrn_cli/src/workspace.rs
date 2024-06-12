/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8PathBuf;

use crate::config::PackageJson;

fn package_json_file() -> Result<Utf8PathBuf> {
    let pwd = ubrn_common::pwd()?;

    let package_json = ubrn_common::resolve(pwd, "package.json")?;
    Ok(package_json.expect("Must be run under a directory containing a package.json file"))
}

pub(crate) fn package_json() -> PackageJson {
    let file = package_json_file().expect("Cannot file package.json");
    ubrn_common::read_from_file(file).expect("Cannot load package.json")
}

pub(crate) fn project_root() -> Result<Utf8PathBuf> {
    let package_json = package_json_file()?;
    let dir = package_json.parent().expect("Must be a directory");
    Ok(dir.into())
}
