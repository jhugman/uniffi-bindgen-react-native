/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::fmt::Display;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser as _;

use crate::cli;
use ubrn_common::{run_cmd, CrateMetadata};

pub fn root_dir() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn integration_fixtures_dir() -> Utf8PathBuf {
    root_dir().join("../../fixtures")
}

pub fn fixtures_dir() -> Utf8PathBuf {
    root_dir().join("fixtures")
}

pub fn run_cli(command_line: impl Display) -> Result<()> {
    let args = cli::CliArgs::try_parse_from(command_line.to_string().split_whitespace())?;
    args.run()
}

pub fn cargo_build(fixture_name: &str, target: &str) -> Result<CrateMetadata> {
    let mut command = std::process::Command::new("cargo");
    let fixture = integration_fixtures_dir().join(fixture_name);
    let manifest_path = fixture.join("Cargo.toml");
    run_cmd(
        command
            .arg("build")
            .arg("--manifest-path")
            .arg(&manifest_path)
            .args(["--target", target]),
    )?;
    CrateMetadata::try_from(manifest_path)
}
