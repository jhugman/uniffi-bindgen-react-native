/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

mod builder;
mod nodes;
mod type_mapping;

pub(crate) use nodes::PlayerFfiModule;
pub use nodes::{LibResolution, TripleStyle};

/// Render a minimal player template for snapshot testing. Hidden from API docs.
#[doc(hidden)]
pub fn render_minimal_for_test(lib_resolution: LibResolution, crate_name: &str) -> String {
    use nodes::{PlayerFfiModule, PlayerSymbols};
    let module = PlayerFfiModule {
        strict_type_checking: false,
        crate_name: crate_name.to_string(),
        lib_resolution,
        symbols: PlayerSymbols {
            rustbuffer_alloc: "ubrn_test_alloc".into(),
            rustbuffer_free: "ubrn_test_free".into(),
            rustbuffer_from_bytes: "ubrn_test_from_bytes".into(),
        },
        functions: Vec::new(),
        callbacks: Vec::new(),
        structs: Vec::new(),
        typed_functions: Vec::new(),
        typed_definitions: Vec::new(),
    };
    super::generate_player_lowlevel_code(module).expect("render")
}
