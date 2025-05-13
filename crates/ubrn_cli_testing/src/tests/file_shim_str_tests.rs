/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use camino::Utf8PathBuf;
use ubrn_common::read_to_string;

use crate::{shim_file_str, start_recording, stop_recording};

#[test]
fn test_file_shim_str() -> anyhow::Result<()> {
    // Create a temporary directory for test files
    let temp_dir = tempfile::tempdir()?;
    let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    // Create a file path that we'll shim with string content
    let file_path = temp_path.join("config.json");

    // The file doesn't exist on disk
    assert!(!file_path.exists());

    // Start recording and set up string content shim
    start_recording();
    shim_file_str(
        "config.json",
        r#"{"name": "test-package", "version": "1.0.0"}"#,
    );

    // Reading the file should return the shimmed string content
    let content = read_to_string(&file_path)?;
    assert_eq!(content, r#"{"name": "test-package", "version": "1.0.0"}"#);

    // Clean up
    stop_recording();
    temp_dir.close()?;

    Ok(())
}

#[test]
fn test_file_shim_str_with_suffix() -> anyhow::Result<()> {
    // Create a temporary directory for test files
    let temp_dir = tempfile::tempdir()?;
    let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    // Create a file path that we'll shim with string content
    let file_path = temp_path.join("some/nested/path/suffix.json");

    // Create the directory structure
    std::fs::create_dir_all(file_path.parent().unwrap())?;

    // Start recording and set up string content shim
    start_recording();
    shim_file_str("suffix.json", r#"{"foo": "bar"}"#);

    // Reading the file should return the shimmed string content
    let content = read_to_string(&file_path)?;
    assert_eq!(content, r#"{"foo": "bar"}"#);

    // Clean up
    stop_recording();
    temp_dir.close()?;

    Ok(())
}

#[test]
fn test_file_shim_str_multiline() -> anyhow::Result<()> {
    // Start recording and set up string content shim with multiline string
    start_recording();

    // Use a multiline string with triple quotes
    let json_content = r#"{
  "name": "test-package",
  "version": "1.0.0",
  "dependencies": {
    "react": "^18.0.0",
    "react-dom": "^18.0.0"
  }
}"#;

    shim_file_str("package.json", json_content);

    // Reading the file should return the shimmed string content
    let content = read_to_string("some/path/package.json")?;
    assert_eq!(content, json_content);

    // Clean up
    stop_recording();

    Ok(())
}
