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
    let p = crate::wasm2::prepare(crate_name, target_tmpdir, "channel");
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
///
/// Mirrors the init sequence in the generated index template
/// (`gen_typescript/templates/index.ts`: openWasm, resolveModule, registerSync,
/// setNativeModule, `default.initialize()`) with the receiver and sync player
/// spliced in between registerSync and setNativeModule; keep the two in step.
fn write_node_bootstrap(ts_dir: &Utf8Path, lib_stem: &str) -> Utf8PathBuf {
    let mut namespaces: Vec<String> = std::fs::read_dir(ts_dir)
        .expect("read ts dir")
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix("-ffi.ts").map(str::to_owned))
        .collect();
    namespaces.sort();
    assert!(!namespaces.is_empty(), "no *-ffi.ts generated in {ts_dir}");

    let glue = ts_dir.join(format!("{lib_stem}_bg.js")).exists();
    let mut src = String::new();
    src.push_str("import { openWasm } from \"@ubjs/wasm\";\n");
    src.push_str("import { createReceiver } from \"@ubjs/worker\";\n");
    src.push_str(
        "import { createSyncPortPair, createSyncPlayer } from \"@ubjs/worker/testing\";\n",
    );
    if glue {
        src.push_str(&format!("import * as glue from \"./{lib_stem}_bg.js\";\n"));
    }
    for ns in &namespaces {
        src.push_str(&format!(
            "import {{ PLAYER_DEFINITIONS as defs_{ns}, setNativeModule as set_{ns} }} from \"./{ns}-ffi.js\";\n\
             import * as mod_{ns} from \"./{ns}.js\";\n"
        ));
    }
    src.push_str(&format!(
        "const wasm = await openWasm(new URL(\"./{lib_stem}.wasm\", import.meta.url){});\n",
        if glue {
            format!(", {{ resolveModule: async (n) => n.endsWith(\"{lib_stem}_bg.js\") ? glue : undefined }}")
        } else {
            String::new()
        }
    ));
    for ns in &namespaces {
        src.push_str(&format!(
            "{{\n  const [rx, tx] = createSyncPortPair();\n\
             \x20 createReceiver(defs_{ns}, wasm.registerSync(defs_{ns}), rx);\n\
             \x20 set_{ns}(createSyncPlayer(defs_{ns}, tx));\n\
             \x20 mod_{ns}.default.initialize();\n}}\n"
        ));
    }
    let path = ts_dir.join("uniffi-bootstrap.channel.ts");
    std::fs::write(&path, src).unwrap_or_else(|e| panic!("write {path}: {e}"));
    path
}
