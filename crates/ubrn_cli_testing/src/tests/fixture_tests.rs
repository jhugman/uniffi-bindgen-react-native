/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{env, fs};

use camino::Utf8PathBuf;
use ubrn_common::{get_recorded_commands, read_to_string};

use crate::{assert_commands, run_cli, shim_file_str, with_fixture, Command};

#[test]
fn test_with_fixture() -> anyhow::Result<()> {
    // Create a temporary directory for test files
    let temp_dir = tempfile::tempdir()?;
    let temp_path = Utf8PathBuf::from_path_buf(temp_dir.path().to_path_buf()).unwrap();

    // Create a mock fixture structure
    let fixture_dir = temp_path.join("fixtures/test-fixture");
    fs::create_dir_all(&fixture_dir)?;

    // Create a mock file within the fixture
    let config_path = fixture_dir.join("config.json");
    fs::write(&config_path, r#"{"name": "test-fixture"}"#)?;

    // Run the test with our fixture
    with_fixture(temp_path.clone(), "fixtures/test-fixture", |fixture_path| {
        // Check that we're in the right directory
        let current_dir = env::current_dir()?;
        let current_dir_path = Utf8PathBuf::try_from(current_dir)?;
        let normalized_current = current_dir_path.as_std_path().canonicalize()?;
        let normalized_fixture = fixture_path.as_std_path().canonicalize()?;
        assert_eq!(normalized_current, normalized_fixture);

        // Test file shim functionality inside with_fixture
        shim_file_str("sample.json", r#"{"foo": "bar"}"#);

        // Read using the shim
        let content = read_to_string("path/to/sample.json")?;
        assert_eq!(content, r#"{"foo": "bar"}"#);

        // Run a command and check that it's recorded
        run_cli("echo test")?;

        let recorded_commands = get_recorded_commands();
        assert_eq!(recorded_commands.len(), 1);
        assert_eq!(recorded_commands[0].program, "echo");
        assert_eq!(recorded_commands[0].args, vec!["test"]);

        Ok(())
    })?;

    // Clean up
    temp_dir.close()?;

    Ok(())
}

#[test]
fn test_run_cli() -> anyhow::Result<()> {
    // Start recording
    crate::start_recording();

    // Run a CLI command
    run_cli("cargo build --release")?;

    // Check the recorded command
    let commands = get_recorded_commands();
    assert_eq!(commands.len(), 1);

    // Verify the command structure
    assert_commands(&[Command::new("cargo").arg("build").arg("--release")]);

    // Stop recording
    crate::stop_recording();

    Ok(())
}
