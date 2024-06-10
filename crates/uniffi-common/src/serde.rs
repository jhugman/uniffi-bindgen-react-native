/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use serde::de::DeserializeOwned;

/// Convenience method to implement Default, but in terms of the `serde(default_value)` rather
/// than `derive(Default)`.
pub fn default<T>() -> T
where
    T: DeserializeOwned,
{
    let empty = serde_json::Value::Object(Default::default());
    serde_json::from_value(empty).unwrap_or_else(|_| panic!("Failed to create default"))
}
