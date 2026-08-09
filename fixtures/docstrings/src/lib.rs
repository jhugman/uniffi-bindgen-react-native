/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

/// Docstrings may contain block comment markers. Typescript doesn't tolerate
/// this, so they must be escaped.
///
/// ```ts
/// const s = identity("hello"); /* nested block comment */
/// ```
#[uniffi::export]
pub fn identity(value: String) -> String {
    value
}

/// A record whose field docstring contains a stray */ marker.
#[derive(uniffi::Record)]
pub struct DocumentedRecord {
    /// Field docs with a */ in them.
    pub name: String,
}

/// Round-trips a record whose field docstring contains a stray */ marker.
#[uniffi::export]
pub fn identity_record(value: DocumentedRecord) -> DocumentedRecord {
    value
}

/// An enum whose variant docstring contains a stray */ marker.
#[derive(uniffi::Enum)]
pub enum DocumentedEnum {
    /// Variant docs with a */ in them.
    First,
    Second,
}

uniffi::setup_scaffolding!();
