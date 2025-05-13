/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

/// Start recording commands (commands will be recorded but not executed)
pub fn start_recording() {
    ubrn_common::enable_command_recording();
    ubrn_common::clear_recorded_commands();
    ubrn_common::clear_recorded_files();
    ubrn_common::clear_file_shims();
}

/// Stop recording commands and clear the recorded commands
pub fn stop_recording() {
    ubrn_common::disable_command_recording();
    ubrn_common::clear_recorded_commands();
    ubrn_common::clear_recorded_files();
    ubrn_common::clear_file_shims();
}
