/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use std::process::Command;

use camino::{Utf8Path, Utf8PathBuf};

use crate::{metadata, paths, run_cmd_quietly, Flavor};

/// The outputs of [`prepare`]: everything a flavor needs to write its own
/// bootstrap and hand off to `run_tsx_with_preload`.
pub(crate) struct Prepared {
    pub(crate) fixture_dir: Utf8PathBuf,
    pub(crate) ts_dir: Utf8PathBuf,
    pub(crate) lib_stem: String,
}

/// Build, generate bindings for, and stage a fixture crate for a
/// player-based (wasm32) flavor, without running any test script.
///
/// Builds the fixture crate once, for `wasm32-unknown-unknown` (the fixture
/// itself depends on `uniffi-runtime-wasm` via a target-specific dep), and
/// generates bindings from that same `.wasm` — `ubrn_bindgen` reads uniffi
/// metadata out of its `UNIFFI_META_*` globals, so no native build is needed.
///
/// The flavor names its own `generated/<flavor>` subdirectory, so the
/// player-based flavors keep separate generated trees despite sharing this
/// code path, and supplies the bindgen switches it needs.
pub(crate) fn prepare(crate_name: &str, target_tmpdir: &str, flavor: Flavor) -> Prepared {
    // Step 0: Check bootstrap
    paths::assert_wasm_bootstrap();

    // Step 1: Build the fixture crate for wasm32 in a shared target dir.
    let lib_stem = metadata::find_cdylib_name(crate_name);
    let fixture_dir = metadata::find_package_dir(crate_name);
    let shared_target_dir = Utf8PathBuf::from(target_tmpdir).join("ubrn-tests-shared/wasm2-target");
    std::fs::create_dir_all(&shared_target_dir).expect("failed to create shared target dir");
    compile_wasm32(crate_name, &shared_target_dir);
    let wasm_file = shared_target_dir
        .join("wasm32-unknown-unknown/release")
        .join(format!("{lib_stem}.wasm"));

    // Step 2: Generate TS bindings from the *unstaged* wasm. Staging strips
    // the `UNIFFI_META_*` exports and may rewrite the module, so metadata has
    // to come off the raw cargo output.
    let generated = fixture_dir.join("generated").join(flavor.as_str());
    let _ = std::fs::remove_dir_all(&generated);
    let ts_dir = generated.join("ts");
    std::fs::create_dir_all(&ts_dir).expect("failed to create ts dir");
    generate_bindings(&wasm_file, &ts_dir, flavor.bindgen_args());

    // Step 3: Stage the wasm next to the TS bindings, with DCE — fixture
    // artifacts are ours, so over-stripping fails a test rather than a user.
    ubrn_common::stage_wasm(&wasm_file, &ts_dir, &lib_stem, true)
        .unwrap_or_else(|e| panic!("staging {wasm_file}: {e:#}"));

    Prepared {
        fixture_dir,
        ts_dir,
        lib_stem,
    }
}

/// Run a fixture test under the Wasm2 (player-based) flavor.
pub fn run_test(crate_name: &str, test_script: &str, target_tmpdir: &str) {
    // Serialize with other flavors for this fixture (they share generated/).
    let _lock = crate::lock_fixture();
    let p = prepare(crate_name, target_tmpdir, Flavor::Wasm2);
    let test_script = Utf8Path::new(test_script);

    // Stand in for the entrypoint a real project would use. Test scripts are
    // shared across flavors, so they import the API module directly and
    // never call `uniffiInitAsync`.
    let bootstrap = write_node_bootstrap(&p.ts_dir, &p.lib_stem);

    let _tsconfig_guard =
        crate::CleanupFile::new(crate::write_fixture_tsconfig(&p.fixture_dir, Flavor::Wasm2));
    crate::run_tsx_with_preload(test_script, &bootstrap);
}

/// Write the preload that initialises the generated bindings, and return its
/// path. The index does the real loading; this only names the asset and calls
/// it, because test scripts are shared across flavors and never call it
/// themselves.
fn write_node_bootstrap(ts_dir: &Utf8Path, lib_stem: &str) -> Utf8PathBuf {
    let path = ts_dir.join("uniffi-bootstrap.node.ts");
    std::fs::write(
        &path,
        format!(
            "import {{ uniffiInitAsync }} from \"./index.js\";\n\
             const t0 = performance.now();\n\
             await uniffiInitAsync(new URL(\"./{lib_stem}.wasm\", import.meta.url));\n\
             // Read by the async benchmark; the same span as AsyncWasm's Worker spawn + initialize.\n\
             (globalThis as any).__ubrnStartupMs = performance.now() - t0;\n"
        ),
    )
    .unwrap_or_else(|e| panic!("write {path}: {e}"));
    path
}

/// The generated namespaces: one `<ns>-ffi.ts` per uniffi namespace.
pub(crate) fn generated_namespaces(ts_dir: &Utf8Path) -> Vec<String> {
    let mut namespaces: Vec<String> = std::fs::read_dir(ts_dir)
        .expect("read ts dir")
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter_map(|name| name.strip_suffix("-ffi.ts").map(str::to_owned))
        .collect();
    namespaces.sort();
    assert!(!namespaces.is_empty(), "no *-ffi.ts generated in {ts_dir}");
    namespaces
}

/// Write the bootstrap a port-backed flavor preloads, and return its path.
///
/// Everything up to the opened wasm is the same for every such flavor: the
/// namespaces are whichever `*-ffi.ts` were generated, and the glue module
/// only exists when wasm-bindgen wrote one. `transport_imports` and
/// `wire_namespace` are the flavor's own: its imports, and the block that
/// puts its transport between `registerSync` and `setNativeModule`.
///
/// Mirrors the init sequence in the generated index template
/// (`gen_typescript/templates/index.ts`: openWasm, resolveModule,
/// registerSync, setNativeModule, `default.initialize()`); keep the two in
/// step.
pub(crate) fn write_player_bootstrap(
    ts_dir: &Utf8Path,
    lib_stem: &str,
    file_name: &str,
    transport_imports: &str,
    wire_namespace: impl Fn(&str) -> String,
) -> Utf8PathBuf {
    let namespaces = generated_namespaces(ts_dir);

    let glue = ts_dir.join(format!("{lib_stem}_bg.js")).exists();
    let mut src = String::new();
    src.push_str("import { openWasm } from \"@ubjs/wasm\";\n");
    src.push_str(transport_imports);
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
        src.push_str(&wire_namespace(ns));
    }
    let path = ts_dir.join(file_name);
    std::fs::write(&path, src).unwrap_or_else(|e| panic!("write {path}: {e}"));
    path
}

/// Generate bindings via the CLI, using the `wasm2` subcommand.
fn generate_bindings(cdylib_path: &Utf8Path, ts_dir: &Utf8Path, extra_args: &[&str]) {
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("-p")
        .arg("uniffi-bindgen-react-native")
        .arg("--")
        .arg("generate")
        .arg("wasm2")
        .arg("bindings");
    cmd.args(extra_args);
    cmd.arg("--library")
        .arg("--ts-dir")
        .arg(ts_dir.as_str())
        .arg(cdylib_path.as_str());
    run_cmd_quietly(&mut cmd);
}

/// `cargo build --lib -p <crate_name> --target wasm32-unknown-unknown`.
///
/// `--lib` because only the cdylib is consumed; a fixture also declaring a
/// `[[bin]]` would otherwise build a multi-megabyte executable nobody reads.
/// No RUSTFLAGS: `ubrn_common::export_growable_table` adds the growable table
/// export to the built module instead.
fn compile_wasm32(crate_name: &str, target_dir: &Utf8Path) {
    run_cmd_quietly(
        Command::new("cargo")
            .env("CARGO_TARGET_DIR", target_dir.as_str())
            .arg("build")
            .arg("--release")
            .arg("--lib")
            .arg("-p")
            .arg(crate_name)
            .arg("--target")
            .arg("wasm32-unknown-unknown"),
    );
}
