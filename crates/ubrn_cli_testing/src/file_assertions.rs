/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use crate::file::{ContentMatcher, File};
use ubrn_common::get_recorded_files;
use ubrn_common::RecordedFile;

/// Helper function to check if file matchers match the recorded files
/// Returns Ok(()) if files match, Err(error_message) with detailed error otherwise
fn check_files(expected_files: &[File]) -> Result<(), String> {
    let recorded = get_recorded_files();

    // If there are no expected files, just return success
    if expected_files.is_empty() {
        return Ok(());
    }

    // If we have no recorded files but expected some, fail
    if recorded.is_empty() {
        return Err(format!(
            "Expected {} files but none were recorded.\nExpected: {:?}",
            expected_files.len(),
            expected_files
        ));
    }

    // Track which files have been matched to handle files in any order
    let mut unmatched_files: Vec<&File> = expected_files.iter().collect();
    let mut matched_recorded_files = vec![false; recorded.len()];

    // Try to match each expected file with a recorded file
    for expected_file in expected_files {
        let mut found_match = false;
        let expected_path = expected_file.path().as_str();

        for (i, recorded_file) in recorded.iter().enumerate() {
            // Skip files that have already been matched
            if matched_recorded_files[i] {
                continue;
            }

            // Check if the recorded file path ends with the expected path (suffix match)
            if recorded_file.path.ends_with(expected_path) {
                // Check content matchers
                if let Some(error) = check_file_content(recorded_file, expected_file) {
                    return Err(error);
                }

                found_match = true;
                matched_recorded_files[i] = true;

                // Remove this file from the unmatched list
                if let Some(pos) = unmatched_files
                    .iter()
                    .position(|f| f.path() == expected_file.path())
                {
                    unmatched_files.remove(pos);
                }

                break;
            }
        }

        if !found_match {
            return Err(format!(
                "No matching file found for expected file: {expected_path}"
            ));
        }
    }

    Ok(())
}

/// Helper function to check if file content matches the expected matchers
/// Returns None if content matches, or Some(error_message) if there's a mismatch
fn check_file_content(recorded: &RecordedFile, expected: &File) -> Option<String> {
    for matcher in expected.content_matchers() {
        match matcher {
            ContentMatcher::Contains(substring) => {
                if !recorded.content.contains(substring) {
                    return Some(format!(
                        "File '{}' should contain '{substring}' but it doesn't.\nContent: {}",
                        recorded.path, recorded.content
                    ));
                }
            }
            ContentMatcher::DoesNotContain(substring) => {
                if recorded.content.contains(substring) {
                    return Some(format!(
                        "File '{}' should not contain '{substring}' but it does.\nContent: {}",
                        recorded.path, recorded.content
                    ));
                }
            }
        }
    }

    None
}

/// Assert that a set of files were written with expected content
/// Panics with a detailed error message if files don't match
pub fn assert_files(expected_files: &[File]) {
    if let Err(message) = check_files(expected_files) {
        panic!("{}", message);
    }
}

/// Check if written files match the expected ones
/// Returns true if they match, false otherwise
/// This is useful for tests that want to verify negative cases
pub fn files_match(expected_files: &[File]) -> bool {
    check_files(expected_files).is_ok()
}
