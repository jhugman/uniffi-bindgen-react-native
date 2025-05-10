/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use ubrn_common::{clear_recorded_commands, disable_command_recording, enable_command_recording};

/// Start recording commands (commands will be recorded but not executed)
pub fn start_recording() {
    enable_command_recording();
    clear_recorded_commands();
}

/// Stop recording commands and clear the recorded commands
pub fn stop_recording() {
    disable_command_recording();
    clear_recorded_commands();
}
