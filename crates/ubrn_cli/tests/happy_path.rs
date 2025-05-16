use std::fmt::Display;

/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Parser;

use ubrn_cli::cli;
use ubrn_cli_testing::{assert_commands, shim_file, with_fixture, Command};

fn root_dir() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn run_cli(command_line: impl Display) -> Result<()> {
    let args = cli::CliArgs::try_parse_from(command_line.to_string().split_whitespace())?;
    args.run()
}

#[test]
fn test_build_command() -> anyhow::Result<()> {
    let fixtures_dir = root_dir();
    with_fixture(fixtures_dir.clone(), "happy-path", |_fixture_dir| {
        // Set up file shims
        shim_file("package.json", fixtures_dir.join("defaults/package.json"));
        shim_file(
            "ubrn.config.yaml",
            fixtures_dir.join("defaults/ubrn.config.yaml"),
        );

        // Run the command under test
        run_cli("ubrn build ios --and-generate --config ubrn.config.yaml")?;

        // Assert the expected commands were executed
        assert_commands(&[
            Command::new("xcodebuild").arg_pair_suffix("-xcframework", ".xcframework")
        ]);

        Ok(())
    })
}
