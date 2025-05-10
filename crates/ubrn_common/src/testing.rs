/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{cell::RefCell, path::PathBuf, process::Command};

use camino::Utf8Path;

thread_local! {
    // Track whether we're recording commands instead of running them
    static RECORDING_MODE: RefCell<bool> = const { RefCell::new(false) };

    // Store the recorded commands
    static RECORDED_COMMANDS: RefCell<Vec<RecordedCommand>> = const { RefCell::new(Vec::new()) };

    // Store the recorded file writes
    static RECORDED_FILES: RefCell<Vec<RecordedFile>> = const { RefCell::new(Vec::new()) };
}

/// A record of a command that would have been executed
#[derive(Debug, Clone)]
pub struct RecordedCommand {
    /// The executable name (like "cargo", "npm", etc.)
    pub program: String,

    /// The arguments that would have been passed
    pub args: Vec<String>,

    /// The working directory for the command, if specified
    pub current_dir: Option<PathBuf>,
}

/// A record of a file that was written
#[derive(Debug, Clone)]
pub struct RecordedFile {
    /// The path to the file
    pub path: String,

    /// The content that was written
    pub content: String,
}

impl RecordedCommand {
    /// Create a new RecordedCommand from a std::process::Command
    fn from_command(cmd: &Command) -> Self {
        // Extract program name
        let program = cmd.get_program().to_string_lossy().to_string();

        // Extract arguments
        let args = cmd
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();

        // Extract working directory if set
        let current_dir = cmd.get_current_dir().map(|p| p.to_path_buf());

        RecordedCommand {
            program,
            args,
            current_dir,
        }
    }
}

/// Enable command recording mode (commands will be recorded but not executed)
pub fn enable_command_recording() {
    RECORDING_MODE.with(|mode| {
        *mode.borrow_mut() = true;
    });
}

/// Disable command recording mode (commands will be executed normally)
pub fn disable_command_recording() {
    RECORDING_MODE.with(|mode| {
        *mode.borrow_mut() = false;
    });
}

/// Check if command recording is currently enabled
pub(crate) fn is_recording_enabled() -> bool {
    RECORDING_MODE.with(|mode| *mode.borrow())
}

/// Record a command instead of executing it
pub(crate) fn record_command(cmd: &Command) {
    let record = RecordedCommand::from_command(cmd);
    RECORDED_COMMANDS.with(|commands| {
        commands.borrow_mut().push(record);
    });
}

/// Get all recorded commands
pub fn get_recorded_commands() -> Vec<RecordedCommand> {
    RECORDED_COMMANDS.with(|commands| commands.borrow().clone())
}

/// Clear the list of recorded commands
pub fn clear_recorded_commands() {
    RECORDED_COMMANDS.with(|commands| {
        commands.borrow_mut().clear();
    });
}

/// Record a file write operation
pub(crate) fn record_file<P: AsRef<Utf8Path>, C: AsRef<[u8]>>(path: P, contents: C) {
    // Only record if recording is enabled
    if !is_recording_enabled() {
        return;
    }

    // Try to convert the content to a string
    if let Ok(content) = std::str::from_utf8(contents.as_ref()) {
        let record = RecordedFile {
            path: path.as_ref().to_string(),
            content: content.to_string(),
        };

        RECORDED_FILES.with(|files| {
            files.borrow_mut().push(record);
        });
    }
}

/// Get all recorded file writes
pub fn get_recorded_files() -> Vec<RecordedFile> {
    RECORDED_FILES.with(|files| files.borrow().clone())
}

/// Clear the list of recorded files
pub fn clear_recorded_files() {
    RECORDED_FILES.with(|files| {
        files.borrow_mut().clear();
    });
}
