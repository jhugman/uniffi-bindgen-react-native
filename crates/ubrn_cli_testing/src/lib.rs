/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod cli;
mod command;
mod command_assertions;
mod file;
mod file_assertions;
mod fixture;
mod recording;

#[cfg(test)]
mod tests;

// Re-export modules
pub use cli::run_cli;
pub use command::Command;
pub use command_assertions::{assert_commands, commands_match};
pub use file::File;
pub use file_assertions::{assert_files, files_match};
pub use fixture::with_fixture;
pub use recording::{start_recording, stop_recording};
pub use ubrn_common::{shim_file_str, shim_path};
