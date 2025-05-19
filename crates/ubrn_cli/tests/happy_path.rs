/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::fmt::Display;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;

use ubrn_cli::cli;
use ubrn_cli_testing::{assert_commands, shim_path, with_fixture, Command};
use ubrn_common::run_cmd;

fn root_dir() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn integration_fixtures_dir() -> Utf8PathBuf {
    root_dir().join("../../fixtures")
}

fn fixtures_dir() -> Utf8PathBuf {
    root_dir().join("fixtures")
}

fn run_cli(command_line: impl Display) -> Result<()> {
    let args = cli::CliArgs::try_parse_from(command_line.to_string().split_whitespace())?;
    args.run()
}

fn cargo_buld(fixture_name: &str, target: &str) -> Result<Utf8PathBuf> {
    let mut command = std::process::Command::new("cargo");
    let fixture = integration_fixtures_dir().join(fixture_name);
    let manifest_path = fixture.join("Cargo.toml");
    run_cmd(
        command
            .arg("build")
            .arg("--manifest-path")
            .arg(manifest_path)
            .args(["--target", target]),
    )?;
    Ok(fixture)
}

#[test]
fn test_build_command() -> Result<()> {
    let rust_fixture = cargo_buld("arithmetic", "aarch64-apple-ios")?;
    let fixtures_dir = fixtures_dir();
    with_fixture(fixtures_dir.clone(), "happy-path", |_fixture_dir| {
        // Set up file shims
        shim_path("package.json", fixtures_dir.join("defaults/package.json"));
        shim_path(
            "ubrn.config.yaml",
            fixtures_dir.join("defaults/ubrn.config.yaml"),
        );
        shim_path("rust/shim/Cargo.toml", rust_fixture.join("Cargo.toml"));
        shim_path("rust/shim", rust_fixture);

        // Run the command under test
        run_cli("ubrn build ios --and-generate --config ubrn.config.yaml")?;

        // Assert the expected commands were executed
        assert_commands(&[
            Command::new("xcodebuild").arg_pair_suffix("-xcframework", ".xcframework")
        ]);

        Ok(())
    })
}
