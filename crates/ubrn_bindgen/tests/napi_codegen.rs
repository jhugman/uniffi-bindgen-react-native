/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Checks that napi's `wrapper-ffi.ts` names each async function's future
//! exports in its table, since `@ubjs/node` runs the poll loop itself.

use ubrn_bindgen::{render_player_lowlevel_for_test, AbiFlavor};

#[test]
fn napi_player_table_renders_an_async_entry() {
    let rendered = render_player_lowlevel_for_test(&AbiFlavor::Napi, false)
        .expect("rendering napi player wrapper-ffi.ts should succeed");
    assert!(
        rendered.contains(
            r#"async: { poll: "ubrn_test_poll", cancel: "ubrn_test_cancel", complete: "ubrn_test_complete", free: "ubrn_test_free", ret: FfiType.UInt32 },"#
        ),
        "expected the table entry to name its future exports:\n{rendered}"
    );
    assert!(
        rendered.contains("): Promise<number>;"),
        "expected the async entry to return a promise of the completed value:\n{rendered}"
    );
}
