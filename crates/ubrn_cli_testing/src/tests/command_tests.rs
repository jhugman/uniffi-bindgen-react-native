/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process;
use ubrn_common::{get_recorded_commands, run_cmd};

use crate::{assert_commands, commands_match, start_recording, stop_recording, Command};

#[test]
fn test_command_recording() {
    // Start recording
    start_recording();

    // Run some commands through the common interface
    let mut cmd1 = process::Command::new("cargo");
    cmd1.arg("build")
        .arg("--target")
        .arg("wasm32-unknown-unknown");
    let _ = run_cmd(&mut cmd1);

    let mut cmd2 = process::Command::new("npm");
    cmd2.arg("install").current_dir("/path/to/project");
    let _ = run_cmd(&mut cmd2);

    // Verify recorded commands
    let recorded = get_recorded_commands();
    assert_eq!(recorded.len(), 2);
    assert_eq!(recorded[0].program, "cargo");
    assert_eq!(
        recorded[0].args,
        vec!["build", "--target", "wasm32-unknown-unknown"]
    );
    assert_eq!(recorded[1].program, "npm");
    assert_eq!(recorded[1].args, vec!["install"]);

    // Stop and clear recording
    stop_recording();
}

#[test]
fn test_command_assertions() {
    // Start recording
    start_recording();

    // Run some commands
    let mut cmd1 = process::Command::new("cargo");
    cmd1.arg("build")
        .arg("--target")
        .arg("wasm32-unknown-unknown");
    let _ = run_cmd(&mut cmd1);

    let mut cmd2 = process::Command::new("xcodebuild");
    cmd2.arg("-create-xframework")
        .arg("-output")
        .arg("/path/to/output.xcframework");
    let _ = run_cmd(&mut cmd2);

    // Test exact command matching - this should not panic
    assert_commands(&[
        Command::new("cargo")
            .arg("build")
            .arg("--target")
            .arg("wasm32-unknown-unknown"),
        Command::new("xcodebuild")
            .arg("-create-xframework")
            .arg("-output")
            .arg("/path/to/output.xcframework"),
    ]);

    // Test matching with argpair - this should not panic
    assert_commands(&[
        Command::new("cargo")
            .arg("build")
            .arg_pair("--target", "wasm32-unknown-unknown"),
        Command::new("xcodebuild")
            .arg("-create-xframework")
            .arg_pair("-output", "/path/to/output.xcframework"),
    ]);

    // Test suffix matching - this should not panic
    assert_commands(&[
        Command::new("cargo")
            .arg("build")
            .arg("--target")
            .arg_suffix("unknown"),
        Command::new("xcodebuild")
            .arg("-create-xframework")
            .arg("-output")
            .arg_suffix(".xcframework"),
    ]);

    // Test pair_suffix matching - this should not panic
    assert_commands(&[
        Command::new("cargo")
            .arg("build")
            .arg_pair_suffix("--target", "unknown"),
        Command::new("xcodebuild")
            .arg("-create-xframework")
            .arg_pair_suffix("-output", ".xcframework"),
    ]);

    // Test with incorrect order (should fail)
    assert!(!commands_match(&[
        Command::new("xcodebuild"),
        Command::new("cargo"),
    ]));

    stop_recording();
}

#[test]
#[should_panic(expected = "Command mismatch at position 0")]
fn test_command_mismatch_panic() {
    // Start recording
    start_recording();

    // Run a cargo command
    let mut cmd = process::Command::new("cargo");
    cmd.arg("build").arg("--release");
    let _ = run_cmd(&mut cmd);

    // This should panic with a helpful error message
    assert_commands(&[
        Command::new("npm").arg("install"), // Wrong command
    ]);

    // We shouldn't get here
    stop_recording();
}

#[test]
fn test_arg_pair_suffix_mismatch() {
    // Start recording
    start_recording();

    // Run a command with a path argument
    let mut cmd = process::Command::new("xcodebuild");
    cmd.arg("-output").arg("/path/to/file.framework");
    let _ = run_cmd(&mut cmd);

    // This should match
    assert!(commands_match(&[
        Command::new("xcodebuild").arg_pair_suffix("-output", ".framework"),
    ]));

    // This should not match - wrong suffix
    assert!(!commands_match(&[
        Command::new("xcodebuild").arg_pair_suffix("-output", ".xcframework"),
    ]));

    stop_recording();
}
