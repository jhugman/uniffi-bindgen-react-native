/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

//! Run a fixture's `--async` script under real Electron, through
//! `@ubjs/electron`: the script in a sandboxed page, the napi player and the
//! native library in a utility process. The bindings are generated as for
//! [`crate::async_napi`]; `electron/runner.mjs` bundles the page and the
//! utility, launches Electron and checks the run.

use std::sync::Once;

use camino::Utf8Path;

use crate::{metadata, paths, Flavor};

/// Also type-checks an app's calls across the packages, once per run.
const LIFECYCLE_SCRIPT: &str = "test_electron_lifecycle.ts";

static BUILD_ELECTRON_PACKAGE: Once = Once::new();

pub fn run_test(crate_name: &str, test_script: &str, _target_tmpdir: &str) {
    // Serialize with other flavors for this fixture (they share generated/).
    let _lock = crate::lock_fixture();
    crate::napi::ensure_napi_runtime_built();
    paths::assert_napi_bootstrap();
    ensure_electron_package_built();

    crate::cargo_build(crate_name);
    let cdylib = metadata::find_cdylib(crate_name);
    let fixture_dir = metadata::find_package_dir(crate_name);
    let generated = fixture_dir
        .join("generated")
        .join(Flavor::AsyncElectron.as_str());
    let _ = std::fs::remove_dir_all(&generated);
    let ts_dir = generated.join("ts");
    std::fs::create_dir_all(&ts_dir).expect("failed to create ts dir");
    crate::wasm2::generate_bindings(&cdylib, &ts_dir, Flavor::AsyncElectron.bindgen_args());

    let script = Utf8Path::new(test_script);
    let stem = script.file_stem().unwrap_or("script");
    // Inside the package, so the utility resolves @ubjs/node, and main
    // @ubjs/electron, as an app's would.
    let out = electron_package_dir()
        .join(".fixture-runs")
        .join(format!("{crate_name}-{stem}"));
    let _ = std::fs::remove_dir_all(&out);
    let runner = paths::repo_root().join("crates/ubrn_fixture_testing/electron/runner.mjs");
    let mut cmd = crate::command("node");
    cmd.arg(runner.as_str())
        .arg(out.as_str())
        .arg(script.as_str())
        .arg(ts_dir.as_str())
        .arg(cdylib.as_str());
    if script.file_name() == Some(LIFECYCLE_SCRIPT) {
        cmd.arg("--typecheck");
    }
    crate::run_cmd(&mut cmd);
}

fn electron_package_dir() -> camino::Utf8PathBuf {
    paths::repo_root().join("channels/electron")
}

/// main loads `/main` and `/preload` from the package's build.
fn ensure_electron_package_built() {
    BUILD_ELECTRON_PACKAGE.call_once(|| {
        crate::run_cmd_quietly(
            crate::command("npm")
                .arg("run")
                .arg("build")
                .current_dir(electron_package_dir().as_str()),
        );
    });
}
