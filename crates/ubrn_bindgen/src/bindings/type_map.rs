/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use uniffi_bindgen::{interface::Type, Component};
use uniffi_meta::AsType;

/// Now vestigial type previously used to handle external types.
///
/// We keep this here for redirection purposes: all calls through filters
/// goes through the `as_type` method, which makes it a useful place to store state
/// and change types.
#[derive(Default, Debug)]
pub(crate) struct TypeMap {}

impl TypeMap {
    pub(crate) fn as_type(&self, as_type: &impl AsType) -> Type {
        as_type.as_type()
    }
}

impl<T> From<&[Component<T>]> for TypeMap {
    fn from(_: &[Component<T>]) -> Self {
        Self::default()
    }
}
