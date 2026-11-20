/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Run a fixture with `--async` bindings over `@ubjs/worker`: the
//! wasm2 player behind a receiver in a node Worker, the generated code in
//! front of a sender on the main thread. [`crate::async_napi`] shares the
//! bootstrap, with its own Worker entry.

use camino::{Utf8Path, Utf8PathBuf};

pub fn run_test(crate_name: &str, test_script: &str, target_tmpdir: &str) {
    let _lock = crate::lock_fixture();
    let p = crate::wasm2::prepare(crate_name, target_tmpdir, crate::Flavor::AsyncWasm);
    let test_script = Utf8Path::new(test_script);
    let bootstrap = write_node_bootstrap(&p.ts_dir, worker_source(&p.ts_dir, &p.lib_stem));
    let _tsconfig_guard = crate::CleanupFile::new(crate::write_fixture_tsconfig(
        &p.fixture_dir,
        crate::Flavor::AsyncWasm,
    ));
    crate::run_tsx_with_preload(test_script, &bootstrap);
}

const WORKER_FILE: &str = "uniffi-worker.async-wasm.ts";
const WORKER_BOOTSTRAP_FILE: &str = "uniffi-worker-bootstrap.async-wasm.mjs";
/// The bindgen's merged table for the whole library, for the Worker's receiver.
pub(crate) const DEFINITIONS_MODULE: &str = "player-definitions";

/// The preload: on the main thread, one `MessageChannel` for the whole
/// library, its receiver end transferred into a Worker, and the bindgen's
/// `uniffiInitAsync(port)` in front of it. `worker_src` is the Worker entry,
/// which puts one receiver, over the flavor's player, on `workerData.port`.
///
/// The Worker is given `execArgv: []` and loads [`WORKER_BOOTSTRAP_FILE`],
/// not this file: tsx's ESM loader hooks don't propagate into a worker
/// thread via inherited `execArgv` (a tsx/Node limitation — the hook
/// registration is per-caller-thread, so replaying `--import` for this file
/// a second time only gets as far as tsx's plain TS-syntax stripping, and
/// its `@ubjs/*`/extensionless imports fail to resolve). Giving the Worker
/// an empty `execArgv` skips that replay outright; see [`write_worker_bootstrap`]
/// for how the Worker registers the hooks it actually needs.
///
/// The Worker is unref'd once initialised: the sender refs its port while a
/// call is outstanding, so a finished script exits.
pub(crate) fn write_node_bootstrap(ts_dir: &Utf8Path, worker_src: String) -> Utf8PathBuf {
    write_generated(ts_dir, WORKER_FILE, worker_src);
    write_worker_bootstrap(ts_dir);

    // No-op today (this file only ever runs on the main thread), but keeps a
    // future execArgv change from making the Worker spawn a Worker.
    let src = format!(
        "import {{ isMainThread, MessageChannel, Worker }} from \"node:worker_threads\";\n\
         import {{ uniffiInitAsync }} from \"./index.port.js\";\n\
         if (isMainThread) {{\n\
         const t0 = performance.now();\n\
         const ch = new MessageChannel();\n\
         const worker = new Worker(new URL(\"./{WORKER_BOOTSTRAP_FILE}\", import.meta.url), {{\n\
         \x20 workerData: {{ port: ch.port1 }},\n\
         \x20 transferList: [ch.port1],\n\
         \x20 // See async_wasm.rs::write_node_bootstrap: skip replaying this file's own\n\
         \x20 // --import inside the Worker.\n\
         \x20 execArgv: [],\n\
         }});\n\
         // A Worker that fails to load or crashes must fail the test, not hang it.\n\
         worker.on(\"error\", (e) => {{\n\
         \x20 throw e;\n\
         }});\n\
         // A Worker that dies without an `error` event must not leave this\n\
         // script waiting for a reply until the test timeout.\n\
         worker.on(\"exit\", (code) => {{\n\
         \x20 if (code !== 0) throw new Error(`uniffi worker exited with ${{code}}`);\n\
         }});\n\
         await uniffiInitAsync(ch.port2);\n\
         (globalThis as any).__ubrnStartupMs = performance.now() - t0;\n\
         worker.unref();\n\
         ch.port2.unref();\n\
         }}\n"
    );
    write_generated(ts_dir, "uniffi-bootstrap.async-wasm.ts", src)
}

/// Writes one generated module into `ts_dir` and returns its path.
fn write_generated(ts_dir: &Utf8Path, name: &str, src: String) -> Utf8PathBuf {
    let path = ts_dir.join(name);
    std::fs::write(&path, src).unwrap_or_else(|e| panic!("write {path}: {e}"));
    path
}

/// Plain-JS shim the Worker actually loads (given `execArgv: []`, it starts
/// with none of tsx's own hooks registered). It registers tsx's loaders
/// itself via the programmatic API, then dynamically imports the real
/// worker module — by then `@ubjs/*` and extensionless imports resolve.
/// The CommonJS hooks are for `@ubjs/node`, whose `lib.js` `require`s
/// `@ubjs/core`: without them that require sees no tsconfig `paths`.
fn write_worker_bootstrap(ts_dir: &Utf8Path) {
    let src = format!(
        "import {{ register as registerCjs }} from \"tsx/cjs/api\";\n\
         import {{ register }} from \"tsx/esm/api\";\n\
         registerCjs();\n\
         register();\n\
         await import(new URL(\"./{WORKER_FILE}\", import.meta.url));\n"
    );
    write_generated(ts_dir, WORKER_BOOTSTRAP_FILE, src);
}

/// The Worker entry: open the wasm and put the one receiver on the port it was
/// handed. Mirrors the init sequence in the generated index template
/// (`gen_typescript/templates/index.ts`): openWasm, resolveModule, registerSync,
/// but once, over the library's merged table.
fn worker_source(ts_dir: &Utf8Path, lib_stem: &str) -> String {
    let glue = ts_dir.join(format!("{lib_stem}_bg.js")).exists();
    let mut src = format!(
        "import {{ workerData }} from \"node:worker_threads\";\n\
         import {{ openWasm }} from \"@ubjs/wasm\";\n\
         import {{ createReceiver }} from \"@ubjs/worker\";\n\
         import {{ PLAYER_DEFINITIONS }} from \"./{DEFINITIONS_MODULE}.js\";\n"
    );
    if glue {
        src.push_str(&format!("import * as glue from \"./{lib_stem}_bg.js\";\n"));
    }
    src.push_str(&format!(
        "const wasm = await openWasm(new URL(\"./{lib_stem}.wasm\", import.meta.url){});\n\
         createReceiver(PLAYER_DEFINITIONS, wasm.registerSync(PLAYER_DEFINITIONS), workerData.port);\n",
        if glue {
            format!(", {{ resolveModule: async (n) => n.endsWith(\"{lib_stem}_bg.js\") ? glue : undefined }}")
        } else {
            String::new()
        }
    ));
    src
}
