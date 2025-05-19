/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::fs;

use camino::Utf8PathBuf;
use ubrn_common::read_to_string;

use crate::{shim_path, start_recording, stop_recording};

#[test]
fn test_file_shim() -> anyhow::Result<()> {
    // Create a temporary directory for test files
    let temp_dir = tempfile::tempdir()?;
    let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    // Create source and target files
    let source_path = temp_path.join("source.txt");
    let target_path = temp_path.join("target.txt");

    fs::write(&source_path, "source content")?;
    fs::write(&target_path, "target content")?;

    // Start recording and set up shim
    start_recording();
    shim_path("source.txt", &target_path);

    // Read source file - should give us target content
    let content = read_to_string(&source_path)?;
    assert_eq!(content, "target content");

    // Clean up
    stop_recording();
    temp_dir.close()?;

    Ok(())
}

#[test]
fn test_file_shim_with_suffix() -> anyhow::Result<()> {
    // Create a temporary directory for test files
    let temp_dir = tempfile::tempdir()?;
    let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    // Create source and target files
    let source_path = temp_path.join("some/nested/path/package.json");
    let target_path = temp_path.join("fixtures/basic/package.json");

    // Create directory structure
    fs::create_dir_all(source_path.parent().unwrap())?;
    fs::create_dir_all(target_path.parent().unwrap())?;

    fs::write(&source_path, r#"{"name": "source-package"}"#)?;
    fs::write(&target_path, r#"{"name": "target-package"}"#)?;

    // Start recording and set up shim
    start_recording();
    shim_path("package.json", &target_path);

    // Read source file - should give us target content
    let content = read_to_string(&source_path)?;
    assert_eq!(content, r#"{"name": "target-package"}"#);

    // Clean up
    stop_recording();
    temp_dir.close()?;

    Ok(())
}

#[test]
fn test_non_existent_shim_path() -> anyhow::Result<()> {
    // Start recording
    start_recording();

    // Shim to a non-existent file
    shim_path("config.json", "/path/that/does/not/exist.json");

    // Read a non-existent file that matches the shim
    let content = read_to_string("/some/path/config.json");

    // When the shimmed path doesn't exist, we get an error.
    assert!(content.is_err());

    // Clean up
    stop_recording();

    Ok(())
}
