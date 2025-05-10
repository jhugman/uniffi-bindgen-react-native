/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
mod command;
mod command_assertions;
mod file;
mod file_assertions;
mod recording;

#[cfg(test)]
mod tests;

// Re-export command, file, and recording modules
pub use command::Command;
pub use command_assertions::{assert_commands, commands_match};
pub use file::File;
pub use file_assertions::{assert_files, files_match};
pub use recording::{start_recording, stop_recording};
