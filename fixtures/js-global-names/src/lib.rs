/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Types named like JavaScript globals. Generated without renaming, they would shadow the
//! globals the generated module relies on (e.g. `Object.freeze`).
use std::sync::Arc;

uniffi::setup_scaffolding!();

/// An object named like the `Object` global.
#[derive(uniffi::Object)]
pub struct Object {
    value: u32,
}

#[uniffi::export]
impl Object {
    #[uniffi::constructor]
    pub fn new(value: u32) -> Arc<Self> {
        Arc::new(Self { value })
    }

    pub fn value(&self) -> u32 {
        self.value
    }
}

/// A record named like the `Map` global.
#[derive(uniffi::Record)]
pub struct Map {
    pub key: String,
    pub value: u32,
}

/// An enum named like the `Symbol` global.
#[derive(uniffi::Enum)]
pub enum Symbol {
    Plus,
    Minus,
}

#[uniffi::export]
pub fn make_map(key: String, value: u32) -> Map {
    Map { key, value }
}

#[uniffi::export]
pub fn apply(symbol: Symbol, a: u32, b: u32) -> u32 {
    match symbol {
        Symbol::Plus => a.wrapping_add(b),
        Symbol::Minus => a.wrapping_sub(b),
    }
}

#[uniffi::export]
pub fn object_value(object: Arc<Object>) -> u32 {
    object.value()
}
