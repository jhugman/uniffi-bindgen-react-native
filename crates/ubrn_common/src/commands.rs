/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use std::process::{Command, Stdio};

// Import needed for our is_recording_enabled and record_command functions
// The actual implementation is in testing.rs
use crate::testing::{is_recording_enabled, record_command};

pub fn run_cmd(cmd: &mut Command) -> Result<()> {
    // If we're in recording mode, just record the command and return success
    if is_recording_enabled() {
        record_command(cmd);
        eprintln!("Recording command: {:?}", *cmd);
        return Ok(());
    }

    // Otherwise, run the command as usual
    eprintln!("Running {:?}", *cmd);
    cmd.stdin(Stdio::inherit());

    let status = cmd.status()?;

    if !status.success() {
        anyhow::bail!("Failed to run command");
    }

    Ok(())
}

/// Run the given command, and only output if there is an error.
pub fn run_cmd_quietly(cmd: &mut Command) -> Result<()> {
    // If we're in recording mode, just record the command and return success
    if is_recording_enabled() {
        record_command(cmd);
        return Ok(());
    }

    // Otherwise, run the command as usual
    cmd.stdin(Stdio::inherit());
    let output = cmd.output().expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Running {:?}", *cmd);
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        anyhow::bail!("FAILED");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use crate::{
        clear_recorded_commands, disable_command_recording, enable_command_recording,
        get_recorded_commands, run_cmd,
    };

    fn assert_command_run_with_args(program: &str, args: &[&str]) -> bool {
        get_recorded_commands().iter().any(|cmd| {
            cmd.program == program && args.iter().all(|arg| cmd.args.iter().any(|a| a == arg))
        })
    }

    fn assert_command_run_in_dir_containing(path_component: &str) -> bool {
        get_recorded_commands().iter().any(|cmd| {
            if let Some(dir) = &cmd.current_dir {
                dir.to_string_lossy().contains(path_component)
            } else {
                false
            }
        })
    }

    #[test]
    fn test_command_recording() {
        // Setup recording mode
        enable_command_recording();
        clear_recorded_commands();

        // Run some commands that would normally execute
        let mut cmd1 = Command::new("cargo");
        cmd1.arg("build");
        run_cmd(&mut cmd1).expect("Should succeed in recording mode");

        let mut cmd2 = Command::new("npm");
        cmd2.args(["install", "--save-dev"]);
        cmd2.current_dir("some/project/path");
        run_cmd(&mut cmd2).expect("Should succeed in recording mode");

        // Get the recorded commands
        let commands = get_recorded_commands();

        // Basic assertions
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].program, "cargo");
        assert_eq!(commands[0].args, vec!["build"]);
        assert_eq!(commands[1].program, "npm");
        assert_eq!(commands[1].args, vec!["install", "--save-dev"]);

        // More ergonomic helper assertions
        assert!(assert_command_run_with_args("cargo", &["build"]));
        assert!(assert_command_run_with_args("npm", &["install"]));
        assert!(assert_command_run_in_dir_containing("some/project"));

        // Cleanup
        disable_command_recording();
    }
}
