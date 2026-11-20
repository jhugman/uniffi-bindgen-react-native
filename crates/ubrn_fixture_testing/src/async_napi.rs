/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Run a fixture with `--async` bindings over `@ubjs/worker`, as
//! [`crate::async_wasm`] does, but with the napi player and the native
//! library behind the receiver. The bindings are `wasm2 --async`, generated
//! from the native cdylib, so no wasm32 build is needed.

use camino::Utf8Path;

use crate::{metadata, paths, Flavor, ForwardSlashPath};

pub fn run_test(crate_name: &str, test_script: &str, _target_tmpdir: &str) {
    // Serialize with other flavors for this fixture (they share generated/).
    let _lock = crate::lock_fixture();
    crate::napi::ensure_napi_runtime_built();
    paths::assert_napi_bootstrap();

    crate::cargo_build(crate_name);
    let cdylib = metadata::find_cdylib(crate_name);

    let fixture_dir = metadata::find_package_dir(crate_name);
    let generated = fixture_dir
        .join("generated")
        .join(Flavor::AsyncNapi.as_str());
    let _ = std::fs::remove_dir_all(&generated);
    let ts_dir = generated.join("ts");
    std::fs::create_dir_all(&ts_dir).expect("failed to create ts dir");
    crate::wasm2::generate_bindings(&cdylib, &ts_dir, Flavor::AsyncNapi.bindgen_args());

    let bootstrap = crate::async_wasm::write_node_bootstrap(&ts_dir, worker_source(&cdylib));
    let _tsconfig_guard = crate::CleanupFile::new(crate::write_fixture_tsconfig(
        &fixture_dir,
        Flavor::AsyncNapi,
    ));
    crate::run_tsx_with_preload(Utf8Path::new(test_script), &bootstrap);
}

/// The Worker entry: open the native library through `@ubjs/node` and put the
/// one receiver on the port it was handed.
fn worker_source(cdylib: &Utf8Path) -> String {
    let definitions = crate::async_wasm::DEFINITIONS_MODULE;
    // Debug quoting escapes `"` and `\`, so it makes a JS string literal.
    let lib_path = format!("{:?}", cdylib.to_forward_slash());
    format!(
        "import {{ workerData }} from \"node:worker_threads\";\n\
         import lib from \"@ubjs/node\";\n\
         import {{ createReceiver }} from \"@ubjs/worker\";\n\
         import {{ PLAYER_DEFINITIONS }} from \"./{definitions}.js\";\n\
         const player = lib.UniffiNativeModule.open({lib_path}).register(PLAYER_DEFINITIONS);\n\
         createReceiver(PLAYER_DEFINITIONS, player, workerData.port);\n"
    )
}
