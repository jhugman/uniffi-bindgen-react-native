/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use ubrn_common::{get_recorded_commands, RecordedCommand};

use crate::{command::ArgMatcher, Command};

/// A helper function that checks if commands match and returns detailed error information
fn check_commands(expected_commands: &[Command]) -> Result<(), String> {
    let recorded = get_recorded_commands();

    // If we have fewer recorded commands than expected, fail immediately
    if recorded.len() < expected_commands.len() {
        return Err(format!(
            "Expected {} commands but only {} were recorded.\nExpected: {:?}\nRecorded: {:?}",
            expected_commands.len(),
            recorded.len(),
            expected_commands,
            recorded
        ));
    }

    for (i, expected) in expected_commands.iter().enumerate() {
        if let Some(mismatch) = command_mismatch(&recorded[i], expected) {
            return Err(format!(
                "Command mismatch at position {}:\n{}\nExpected: {:?}\nRecorded: {:?}",
                i, mismatch, expected, recorded[i]
            ));
        }
    }

    Ok(())
}

/// Assert that a sequence of commands were run in the specified order
/// Panics with a detailed error message if commands don't match
pub fn assert_commands(expected_commands: &[Command]) {
    if let Err(message) = check_commands(expected_commands) {
        panic!("{}", message);
    }
}

/// Check if executed commands match the expected ones
/// Returns true if they match, false otherwise
/// This is useful for tests that want to verify negative cases
pub fn commands_match(expected_commands: &[Command]) -> bool {
    check_commands(expected_commands).is_ok()
}

/// Check if a recorded command matches an expected command pattern
/// Returns None if matching, or Some(error_message) with details about the mismatch
fn command_mismatch(recorded: &RecordedCommand, expected: &Command) -> Option<String> {
    // Check program name
    if recorded.program != expected.program() {
        return Some(format!(
            "Program name mismatch: expected '{}', got '{}'",
            expected.program(),
            recorded.program
        ));
    }

    // Check working directory if specified
    if let Some(expected_dir) = expected.get_cwd() {
        match &recorded.current_dir {
            Some(actual_dir) => {
                let expected_str = expected_dir.to_string_lossy();
                let actual_str = actual_dir.to_string_lossy();

                // If it's not an exact match, check if it's a suffix match
                if expected_str != actual_str && !actual_str.contains(&expected_str.to_string()) {
                    return Some(format!(
                        "Working directory mismatch: expected path containing '{}', got '{}'",
                        expected_str, actual_str
                    ));
                }
            }
            None => {
                return Some(format!(
                    "Working directory mismatch: expected path containing '{}', but no working directory was set",
                    expected_dir.to_string_lossy()
                ));
            }
        }
    }

    // Check arguments - we need to consider the order and the different matcher types
    let mut arg_idx = 0;
    let recorded_args = &recorded.args;

    for matcher in expected.args().iter() {
        match matcher {
            ArgMatcher::Exact(expected_arg) => {
                if arg_idx >= recorded_args.len() {
                    return Some(format!(
                        "Missing argument at position {}: expected '{}'",
                        arg_idx, expected_arg
                    ));
                }
                if recorded_args[arg_idx] != *expected_arg {
                    return Some(format!(
                        "Argument mismatch at position {}: expected '{}', got '{}'",
                        arg_idx, expected_arg, recorded_args[arg_idx]
                    ));
                }
                arg_idx += 1;
            }
            ArgMatcher::Suffix(suffix) => {
                if arg_idx >= recorded_args.len() {
                    return Some(format!(
                        "Missing argument at position {}: expected argument ending with '{}'",
                        arg_idx, suffix
                    ));
                }
                if !recorded_args[arg_idx].ends_with(suffix) {
                    return Some(format!(
                        "Argument suffix mismatch at position {}: expected suffix '{}', got '{}'",
                        arg_idx, suffix, recorded_args[arg_idx]
                    ));
                }
                arg_idx += 1;
            }
            ArgMatcher::Pair(key, value) => {
                if arg_idx >= recorded_args.len() {
                    return Some(format!(
                        "Missing argument pair at position {}: expected key '{}' with value '{}'",
                        arg_idx, key, value
                    ));
                }
                if arg_idx + 1 >= recorded_args.len() {
                    return Some(format!(
                        "Incomplete argument pair at position {}: got key '{}' but missing value (expected '{}')",
                        arg_idx,
                        recorded_args[arg_idx],
                        value
                    ));
                }
                if recorded_args[arg_idx] != *key {
                    return Some(format!(
                        "Argument pair key mismatch at position {}: expected '{}', got '{}'",
                        arg_idx, key, recorded_args[arg_idx]
                    ));
                }
                if recorded_args[arg_idx + 1] != *value {
                    return Some(format!(
                        "Argument pair value mismatch at position {}: for key '{}', expected '{}', got '{}'",
                        arg_idx + 1, key, value, recorded_args[arg_idx + 1]
                    ));
                }
                arg_idx += 2;
            }
            ArgMatcher::PairSuffix(key, value_suffix) => {
                if arg_idx >= recorded_args.len() {
                    return Some(format!(
                        "Missing argument pair at position {}: expected key '{}' with value ending with '{}'",
                        arg_idx, key, value_suffix
                    ));
                }
                if arg_idx + 1 >= recorded_args.len() {
                    return Some(format!(
                        "Incomplete argument pair at position {}: got key '{}' but missing value (expected to end with '{}')",
                        arg_idx,
                        recorded_args[arg_idx],
                        value_suffix
                    ));
                }
                if recorded_args[arg_idx] != *key {
                    return Some(format!(
                        "Argument pair key mismatch at position {}: expected '{}', got '{}'",
                        arg_idx, key, recorded_args[arg_idx]
                    ));
                }
                if !recorded_args[arg_idx + 1].ends_with(value_suffix) {
                    return Some(format!(
                        "Argument pair value suffix mismatch at position {}: for key '{}', expected suffix '{}', got '{}'",
                        arg_idx + 1, key, value_suffix, recorded_args[arg_idx + 1]
                    ));
                }
                arg_idx += 2;
            }
        }
    }

    None
}
