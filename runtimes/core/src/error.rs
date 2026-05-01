/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Core error type shared across all APIs.

use std::fmt;

#[derive(Debug)]
pub enum Error {
    LibraryOpen(String),
    SymbolNotFound(String),
    UnknownFunction(String),
    UnknownCallback(String),
    UnknownStruct(String),
    UnsupportedType(String),
    Unloading,
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::LibraryOpen(s) => write!(f, "library open failed: {s}"),
            Error::SymbolNotFound(s) => write!(f, "symbol not found: {s}"),
            Error::UnknownFunction(s) => write!(f, "unknown function: {s}"),
            Error::UnknownCallback(s) => write!(f, "unknown callback: {s}"),
            Error::UnknownStruct(s) => write!(f, "unknown struct: {s}"),
            Error::UnsupportedType(s) => write!(f, "unsupported type: {s}"),
            Error::Unloading => write!(f, "module is unloading or unloaded"),
            Error::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
