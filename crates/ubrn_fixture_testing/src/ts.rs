/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command;

use crate::{paths, typescript, Flavor};
use camino::{Utf8Path, Utf8PathBuf};

/// Run a framework TypeScript test (no fixture crate).
///
/// Called from proc-macro-generated `#[test]` functions.
pub fn run_test(test_script: &str, flavor: Flavor, target_tmpdir: &str) {
    crate::set_target_dir(target_tmpdir);

    let test_script = Utf8Path::new(test_script);
    let test_stem = test_script.file_stem().unwrap_or("test");
    let flavor_name = flavor.as_str();

    let out_dir =
        Utf8PathBuf::from(target_tmpdir).join(format!("ubrn-tests/ts-{test_stem}-{flavor_name}"));
    std::fs::create_dir_all(&out_dir).expect("failed to create output dir");

    match flavor {
        Flavor::Jsi => {
            paths::assert_jsi_bootstrap();
            let bundle_path = out_dir
                .join("bundles")
                .join(format!("{test_stem}.bundle.js"));
            // Cache: skip tsc+metro if the bundle exists and the source hasn't changed.
            let cache_key = ts_cache_key(test_script);
            if crate::is_cache_valid(&out_dir, &cache_key) && bundle_path.exists() {
                run_test_runner_no_lib(&bundle_path);
            } else {
                let bundle = typescript::prepare_for_jsi(test_script, &out_dir, None);
                crate::write_cache_stamp(&out_dir, &cache_key);
                run_test_runner_no_lib(&bundle);
            }
        }
        Flavor::Wasm => {
            paths::assert_wasm_bootstrap();
            // WASM framework tests run directly via tsx
            crate::run_tsx(test_script);
        }
    }
}

fn ts_cache_key(test_script: &Utf8Path) -> String {
    let script_hash = crate::file_content_hash(test_script.as_std_path());
    format!("ts:{script_hash}")
}

fn run_test_runner_no_lib(bundle: &Utf8Path) {
    let runner = paths::test_runner_binary();
    crate::run_cmd(Command::new(runner.as_str()).arg(bundle.as_str()));
}
