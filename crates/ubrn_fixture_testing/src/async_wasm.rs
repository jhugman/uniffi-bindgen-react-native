/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Run a fixture with `--async` bindings over `@ubjs/worker`: the
//! wasm2 player behind a receiver in a node Worker, the generated code in
//! front of a sender on the main thread.

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

const WORKER_FILE: &str = "uniffi-worker.async-wasm.ts";
const WORKER_BOOTSTRAP_FILE: &str = "uniffi-worker-bootstrap.async-wasm.mjs";

/// The preload: on the main thread, one `MessageChannel` for the whole
/// library, its receiver end transferred into a Worker, one sender in front of
/// every generated module.
///
/// There is one channel, not one per namespace: the generated clone of an
/// object is fire-and-forget and must arrive before the call that consumes
/// the handle, and Node orders messages within a port, not across ports.
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
fn write_node_bootstrap(ts_dir: &Utf8Path, lib_stem: &str) -> Utf8PathBuf {
    let namespaces = crate::wasm2::generated_namespaces(ts_dir);
    write_definitions(ts_dir, &namespaces);
    write_worker(ts_dir, lib_stem);
    write_worker_bootstrap(ts_dir);

    let mut src = format!(
        "import {{ isMainThread, MessageChannel, Worker }} from \"node:worker_threads\";\n\
         import {{ createSender }} from \"@ubjs/worker\";\n\
         import {{ DEFINITIONS }} from \"./{DEFINITIONS_MODULE}.js\";\n"
    );
    for ns in &namespaces {
        src.push_str(&format!(
            "import {{ setNativeModule as set_{ns} }} from \"./{ns}-ffi.js\";\n\
             import * as mod_{ns} from \"./{ns}.js\";\n"
        ));
    }
    // No-op today (this file only ever runs on the main thread), but keeps a
    // future execArgv change from making the Worker spawn a Worker.
    src.push_str("if (isMainThread) {\n");
    src.push_str("const t0 = performance.now();\n");
    src.push_str(&format!(
        "const ch = new MessageChannel();\n\
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
         const sender = createSender(DEFINITIONS as any, ch.port2);\n"
    ));
    for ns in &namespaces {
        src.push_str(&format!(
            "set_{ns}(sender as any);\n\
             await mod_{ns}.default.initialize();\n"
        ));
    }
    src.push_str("(globalThis as any).__ubrnStartupMs = performance.now() - t0;\n");
    src.push_str("worker.unref();\n");
    src.push_str("ch.port2.unref();\n");
    src.push_str("}\n");

    let path = ts_dir.join("uniffi-bootstrap.async-wasm.ts");
    std::fs::write(&path, src).unwrap_or_else(|e| panic!("write {path}: {e}"));
    path
}

const DEFINITIONS_MODULE: &str = "uniffi-definitions.async-wasm";

/// One definitions object for the whole library, imported by both sides of the
/// channel. Names are namespace-prefixed, and a name shared between namespaces
/// (an external type's functions) has an identical definition in each, so the
/// merge is order-independent.
fn write_definitions(ts_dir: &Utf8Path, namespaces: &[String]) {
    let mut src = String::new();
    for ns in namespaces {
        src.push_str(&format!(
            "import {{ PLAYER_DEFINITIONS as defs_{ns} }} from \"./{ns}-ffi.js\";\n"
        ));
    }
    let all: Vec<String> = namespaces.iter().map(|ns| format!("defs_{ns}")).collect();
    src.push_str(&format!(
        "const all = [{}];\n\
         export const DEFINITIONS = {{\n\
         \x20 symbols: all[0].symbols,\n\
         \x20 functions: Object.assign({{}}, ...all.map((d) => d.functions)),\n\
         \x20 callbacks: Object.assign({{}}, ...all.map((d) => d.callbacks)),\n\
         \x20 structs: Object.assign({{}}, ...all.map((d) => d.structs)),\n\
         }};\n",
        all.join(", ")
    ));
    let path = ts_dir.join(format!("{DEFINITIONS_MODULE}.ts"));
    std::fs::write(&path, src).unwrap_or_else(|e| panic!("write {path}: {e}"));
}

/// Plain-JS shim the Worker actually loads (given `execArgv: []`, it starts
/// with none of tsx's own hooks registered). It registers tsx's loader
/// itself via the programmatic API, then dynamically imports the real
/// worker module — by then `@ubjs/*` and extensionless imports resolve.
fn write_worker_bootstrap(ts_dir: &Utf8Path) {
    let src = format!(
        "import {{ register }} from \"tsx/esm/api\";\n\
         register();\n\
         await import(new URL(\"./{WORKER_FILE}\", import.meta.url));\n"
    );
    let path = ts_dir.join(WORKER_BOOTSTRAP_FILE);
    std::fs::write(&path, src).unwrap_or_else(|e| panic!("write {path}: {e}"));
}

/// The Worker entry: open the wasm and put the one receiver on the port it was
/// handed. Mirrors the init sequence in the generated index template
/// (`gen_typescript/templates/index.ts`): openWasm, resolveModule, registerSync.
fn write_worker(ts_dir: &Utf8Path, lib_stem: &str) {
    let glue = ts_dir.join(format!("{lib_stem}_bg.js")).exists();
    let mut src = format!(
        "import {{ workerData }} from \"node:worker_threads\";\n\
         import {{ openWasm }} from \"@ubjs/wasm\";\n\
         import {{ createReceiver }} from \"@ubjs/worker\";\n\
         import {{ DEFINITIONS }} from \"./{DEFINITIONS_MODULE}.js\";\n"
    );
    if glue {
        src.push_str(&format!("import * as glue from \"./{lib_stem}_bg.js\";\n"));
    }
    src.push_str(&format!(
        "const wasm = await openWasm(new URL(\"./{lib_stem}.wasm\", import.meta.url){});\n\
         createReceiver(DEFINITIONS, wasm.registerSync(DEFINITIONS), workerData.port);\n",
        if glue {
            format!(", {{ resolveModule: async (n) => n.endsWith(\"{lib_stem}_bg.js\") ? glue : undefined }}")
        } else {
            String::new()
        }
    ));
    let path = ts_dir.join(WORKER_FILE);
    std::fs::write(&path, src).unwrap_or_else(|e| panic!("write {path}: {e}"));
}
