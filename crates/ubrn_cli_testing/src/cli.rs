/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command as StdCommand;

use anyhow::{Context, Result};

/// Run a CLI command and return the result.
///
/// This function takes a command string, splits it into a program and arguments,
/// and runs it. When recording is enabled, the command will be recorded but not executed.
///
/// # Arguments
///
/// * `cmd_str` - The command string to run (e.g., "build ios --config config.yaml")
///
/// # Returns
///
/// Returns a `Result` indicating success or failure
///
/// # Example
///
/// ```no_run
/// use ubrn_cli_testing::run_cli;
///
/// fn test_cli_command() -> anyhow::Result<()> {
///     run_cli("build ios --and-generate --config ubrn.config.yaml")?;
///     Ok(())
/// }
/// ```
pub fn run_cli(cmd_str: &str) -> Result<()> {
    let parts: Vec<&str> = cmd_str.split_whitespace().collect();

    if parts.is_empty() {
        return Err(anyhow::anyhow!("Empty command string"));
    }

    let program = parts[0];
    let args = &parts[1..];
    let mut cmd = StdCommand::new(program);
    cmd.args(args);

    // Use the ubrn_common command execution, which handles recording
    ubrn_common::run_cmd(&mut cmd).with_context(|| format!("Failed to execute command: {cmd_str}"))
}
