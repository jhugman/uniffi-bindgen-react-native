/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

mod entrypoint;
pub(crate) mod extensions;
pub(crate) mod gen_cpp;
#[cfg(feature = "wasm")]
pub(crate) mod gen_rust;
pub(crate) mod gen_typescript;
pub(crate) mod metadata;
pub(crate) mod type_map;

pub use self::entrypoint::generate_entrypoint;
