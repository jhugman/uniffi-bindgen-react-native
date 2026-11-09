/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// This crate does not use UniFFI, but it exposes some types which are used by crates which do.
// This type is referenced as an "external" type as a dictionary.
pub struct RemoteRecord {
    pub sval: String,
}

pub struct RemoteObject {
    pub sval: String,
}

#[non_exhaustive]
pub enum RemoteEnum {
    One,
    Two,
}

// This type is referenced as an "external" type as an interface.
impl RemoteObject {
    pub fn new(sval: String) -> Self {
        RemoteObject { sval }
    }

    pub fn value(&self) -> String {
        self.sval.clone()
    }
}
