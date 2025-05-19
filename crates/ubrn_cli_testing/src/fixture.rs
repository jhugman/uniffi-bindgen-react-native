/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::env;

use anyhow::Result;
use camino::Utf8PathBuf;

use crate::recording::{start_recording, stop_recording};

/// Run a test with a specific fixture.
///
/// This function:
/// 1. Changes to the given fixture directory
/// 2. Starts recording
/// 3. Runs the given closure with the fixture directory path
/// 4. Stops recording
/// 5. Changes back to the original directory
///
/// # Arguments
///
/// * `root_dir` - The root directory of the project
/// * `fixture_name` - The name/path of the fixture under test
/// * `test_fn` - The test function to run
///
/// # Returns
///
/// Returns the result of the test function
///
/// # Example
///
/// ```no_run
/// use ubrn_cli_testing::{with_fixture, Command, assert_commands, run_cli, shim_path};
/// use camino::Utf8PathBuf;
///
/// fn test_build_command() -> anyhow::Result<()> {
///     with_fixture(Utf8PathBuf::from("path/to/project"), "fixtures/my-fixture", |fixture_dir| {
///         // Set up file shims
///         shim_path("package.json", fixture_dir.join("basic/package.json"));
///
///         // Run the command under test
///         run_cli("build ios --and-generate --config ubrn.config.yaml")?;
///
///         // Assert the expected commands were executed
///         assert_commands(&[
///             Command::new("xcodebuild")
///                 .arg_pair_suffix("-xcframework", ".xcframework")
///         ]);
///
///         Ok(())
///     })
/// }
/// ```
pub fn with_fixture<F, T>(root_dir: Utf8PathBuf, fixture_name: &str, test_fn: F) -> Result<T>
where
    F: FnOnce(Utf8PathBuf) -> Result<T>,
{
    // Save current directory so we can return to it
    let current_dir = env::current_dir()?;

    // Build the fixture directory path
    let fixture_dir = root_dir.join(fixture_name);

    // Change to the fixture directory
    env::set_current_dir(&fixture_dir)?;

    // Start recording
    start_recording();

    // Run the test function with the fixture directory
    let result = test_fn(fixture_dir.clone());

    // Stop recording
    stop_recording();

    // Change back to the original directory
    env::set_current_dir(current_dir)?;

    result
}
