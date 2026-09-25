/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Run a fixture with `--async` bindings over `@ubjs/worker`: the
//! wasm2 player behind a receiver, the generated code in front of a sender,
//! both in one Node process over a real `MessageChannel`.

use camino::{Utf8Path, Utf8PathBuf};

pub fn run_test(crate_name: &str, test_script: &str, target_tmpdir: &str) {
    let _lock = crate::lock_fixture();
    let p = crate::wasm2::prepare(crate_name, target_tmpdir, crate::Flavor::AsyncWasm);
    let test_script = Utf8Path::new(test_script);
    let bootstrap = write_node_bootstrap(&p.ts_dir, &p.lib_stem);
    let _tsconfig_guard = crate::CleanupFile::new(crate::write_fixture_tsconfig(
        &p.fixture_dir,
        crate::Flavor::AsyncWasm,
    ));
    crate::run_tsx_with_preload(test_script, &bootstrap);
}

/// One receiver and one sender per generated namespace, each on its own
/// `MessageChannel`.
///
/// Both ports are unref'd once initialised: the sender refs its port while a
/// call is outstanding, so a finished script exits.
fn write_node_bootstrap(ts_dir: &Utf8Path, lib_stem: &str) -> Utf8PathBuf {
    crate::wasm2::write_player_bootstrap(
        ts_dir,
        lib_stem,
        "uniffi-bootstrap.async-wasm.ts",
        "import { MessageChannel } from \"node:worker_threads\";\n\
         import { createReceiver, createSender } from \"@ubjs/worker\";\n",
        |ns| {
            format!(
                "{{\n  const {{ port1, port2 }} = new MessageChannel();\n\
                 \x20 createReceiver(defs_{ns}, wasm.registerSync(defs_{ns}), port1);\n\
                 \x20 set_{ns}(createSender(defs_{ns}, port2));\n\
                 \x20 await mod_{ns}.default.initialize();\n\
                 \x20 port1.unref();\n\
                 \x20 port2.unref();\n}}\n"
            )
        },
    )
}
