/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use crate::{assert_files, files_match, start_recording, stop_recording, File};
use ubrn_common::{clear_recorded_files, get_recorded_files, write_file};

#[test]
fn test_file_recording() {
    // Start recording
    start_recording();

    // Write some files
    write_file("/path/to/file.kt", "This is a test file with some content").unwrap();
    write_file(
        "/another/path/example.txt",
        "Another example\nwith multiple lines",
    )
    .unwrap();

    // Get recorded files to verify they were captured
    let recorded = get_recorded_files();
    assert_eq!(recorded.len(), 2);
    assert!(recorded[0].path.ends_with("file.kt"));
    assert!(recorded[1].path.ends_with("example.txt"));

    // Clean up
    stop_recording();
    clear_recorded_files();
}

#[test]
fn test_file_assertions() {
    start_recording();

    write_file("/path/to/file.kt", "This is a test file with some content").unwrap();

    // Test successful assertions
    assert_files(&[File::new("file.kt")
        .contains("test file")
        .contains("content")
        .does_not_contain("missing text")]);

    // Test files_match function
    assert!(files_match(&[File::new("file.kt").contains("test file")]));

    assert!(!files_match(&[
        File::new("file.kt").contains("non-existent content")
    ]));

    // Test suffix path matching
    assert_files(&[File::new("/to/file.kt").contains("test file")]);

    clear_recorded_files();
    stop_recording();
}

#[test]
#[should_panic(expected = "should contain")]
fn test_file_assertion_fails_when_content_missing() {
    start_recording();

    write_file("/path/to/file.kt", "This is a test file with some content").unwrap();

    // This should fail because the file doesn't contain "missing content"
    assert_files(&[File::new("file.kt").contains("missing content")]);

    clear_recorded_files();
    stop_recording();
}

#[test]
#[should_panic(expected = "should not contain")]
fn test_file_assertion_fails_when_unwanted_content_present() {
    start_recording();

    write_file("/path/to/file.kt", "This is a test file with some content").unwrap();

    // This should fail because the file contains "test file"
    assert_files(&[File::new("file.kt").does_not_contain("test file")]);

    clear_recorded_files();
    stop_recording();
}

#[test]
#[should_panic(expected = "No matching file found")]
fn test_file_assertion_fails_when_file_missing() {
    start_recording();

    write_file("/path/to/file.kt", "This is a test file with some content").unwrap();

    // This should fail because there's no file with this path
    assert_files(&[File::new("nonexistent.txt").contains("anything")]);

    clear_recorded_files();
    stop_recording();
}
