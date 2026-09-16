/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Run a fixture through `@ubjs/worker`: the wasm2 player behind a
//! receiver, the generated code in front of a synchronous player shim, both
//! in one Node process over an in-process synchronous port.

use camino::{Utf8Path, Utf8PathBuf};

pub fn run_test(crate_name: &str, test_script: &str, target_tmpdir: &str) {
    let _lock = crate::lock_fixture();
    let p = crate::wasm2::prepare(crate_name, target_tmpdir, crate::Flavor::Channel);
    let test_script = Utf8Path::new(test_script);
    let bootstrap = write_node_bootstrap(&p.ts_dir, &p.lib_stem);
    let _tsconfig_guard = crate::CleanupFile::new(crate::write_fixture_tsconfig(
        &p.fixture_dir,
        crate::Flavor::Channel,
    ));
    crate::run_tsx_with_preload(test_script, &bootstrap);
}

/// One receiver and one sync player per generated namespace, each on its own
/// port pair: a receiver answers every `call` it sees, so two on one port
/// would fight over unknown names.
fn write_node_bootstrap(ts_dir: &Utf8Path, lib_stem: &str) -> Utf8PathBuf {
    crate::wasm2::write_player_bootstrap(
        ts_dir,
        lib_stem,
        "uniffi-bootstrap.channel.ts",
        "import { createReceiver } from \"@ubjs/worker\";\n\
         import { createSyncPortPair, createSyncPlayer } from \"@ubjs/worker/testing\";\n",
        |ns| {
            format!(
                "{{\n  const [rx, tx] = createSyncPortPair();\n\
                 \x20 createReceiver(defs_{ns}, wasm.registerSync(defs_{ns}), rx);\n\
                 \x20 set_{ns}(createSyncPlayer(defs_{ns}, tx));\n\
                 \x20 mod_{ns}.default.initialize();\n}}\n"
            )
        },
    )
}
