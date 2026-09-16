/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Named for the channel, not the flavor: an AsyncJsi flavor runs the same
// script once a native transport exists.
ubrn_macros::build_foreign_language_testcases! {
    "tests/bindings/test_async_channel.ts" => [AsyncWasm],
}
