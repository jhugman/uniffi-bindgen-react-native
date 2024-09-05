/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::sync::Arc;

use matrix_sdk_ffi::client_builder::ClientBuilder;

#[uniffi::export]
pub fn get_matrix_client_builder() -> Arc<ClientBuilder> {
    ClientBuilder::new()
}

uniffi::setup_scaffolding!();
