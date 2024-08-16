/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// This crate does not use UniFFI, but it exposes some types which are used by crates which do.
// This type is referenced as an "external" type as a dictionary.
pub struct ExternalCrateDictionary {
    pub sval: String,
}

pub struct ExternalCrateInterface {
    pub sval: String,
}

#[non_exhaustive]
pub enum ExternalCrateNonExhaustiveEnum {
    One,
    Two,
}

// This type is referenced as an "external" type as an interface.
impl ExternalCrateInterface {
    pub fn new(sval: String) -> Self {
        ExternalCrateInterface { sval }
    }

    pub fn value(&self) -> String {
        self.sval.clone()
    }
}
