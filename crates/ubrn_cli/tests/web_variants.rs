/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use anyhow::Result;

use ubrn_cli::test_utils::{cargo_build, fixtures_dir, run_cli};
use ubrn_cli_testing::{assert_commands, assert_files, shim_path, with_fixture, Command, File};

#[test]
fn test_release() -> Result<()> {
    let target_crate = cargo_build("arithmetic")?;
    let fixtures_dir = fixtures_dir();
    with_fixture(fixtures_dir.clone(), "defaults", |_fixture_dir| {
        // Set up file shims
        shim_path("package.json", fixtures_dir.join("defaults/package.json"));
        shim_path(
            "ubrn.config.yaml",
            fixtures_dir.join("defaults/ubrn.config.yaml"),
        );
        shim_path("rust/shim/Cargo.toml", target_crate.manifest_path());
        shim_path("rust/shim", target_crate.project_root());

        shim_path("rust_modules/wasm/Cargo.toml", target_crate.manifest_path());
        shim_path(
            "libarithmetical.a",
            target_crate.library_path(None, "debug", None),
        );

        // Run the command under test
        run_cli("ubrn build web --config ubrn.config.yaml --release")?;

        // Assert the expected commands were executed
        assert_commands(&[
            Command::new("cargo")
                .arg("build")
                .arg_pair_suffix("--manifest-path", "fixtures/arithmetic/Cargo.toml"),
            Command::new("prettier"),
            Command::new("cargo")
                .arg("build")
                .arg_pair_suffix("--manifest-path", "rust_modules/wasm/Cargo.toml")
                .arg_pair("--target", "wasm32-unknown-unknown")
                .arg_pair("--profile", "release"),
            Command::new("wasm-bindgen")
                .arg_pair("--target", "web")
                .arg("--omit-default-module-path")
                .arg_pair("--out-name", "index")
                .arg_pair_suffix("--out-dir", "src/generated/wasm-bindgen")
                .arg_suffix("wasm32-unknown-unknown/release/arithmetical.wasm"),
        ]);

        Ok(())
    })
}

#[test]
fn test_monorepo() -> Result<()> {
    // Still using this target for initial build
    let target_crate = cargo_build("arithmetic")?;
    let fixtures_dir = fixtures_dir();
    with_fixture(fixtures_dir.clone(), "defaults", |_fixture_dir| {
        // Set up file shims
        shim_path("package.json", fixtures_dir.join("defaults/package.json"));
        shim_path(
            "ubrn.config.yaml",
            fixtures_dir.join("defaults/ubrn-wasm.monorepo.config.yaml"),
        );
        shim_path("rust/shim/Cargo.toml", target_crate.manifest_path());
        shim_path("rust/shim", target_crate.project_root());

        shim_path(
            "rust/crates/wasm-crate/Cargo.toml",
            target_crate.manifest_path(),
        );
        shim_path(
            "libarithmetical.a",
            target_crate.library_path(None, "debug", None),
        );

        run_cli("ubrn build web --config ubrn.config.yaml")?;

        // Assert the expected commands were executed
        assert_commands(&[
            Command::new("cargo")
                .arg("build")
                .arg_pair_suffix("--manifest-path", "fixtures/arithmetic/Cargo.toml")
                .arg_pair("--features", "wasm32_only"),
            Command::new("prettier"),
            Command::new("cargo")
                .arg("build")
                .arg_pair_suffix("--manifest-path", "rust/crates/wasm-crate/Cargo.toml")
                .arg_pair("--target", "wasm32-unknown-unknown"),
            Command::new("wasm-bindgen")
                .arg_pair("--target", "web")
                .arg("--omit-default-module-path")
                .arg_pair("--out-name", "index")
                .arg_pair_suffix("--out-dir", "src/generated/wasm-bindgen")
                .arg_suffix("wasm32-unknown-unknown/debug/arithmetical.wasm"),
        ]);

        assert_files(&[
            File::new("rust/crates/wasm-crate/Cargo.toml")
                .does_not_contain("[workspace]")
                .contains("uniffi-example-arithmetic = { path = \"../../shim\", features = [\"wasm32_only\"] }"),
            // other files elided.
        ]);

        Ok(())
    })
}

#[test]
fn test_multi_features() -> Result<()> {
    // Still using this target for initial build
    let target_crate = cargo_build("arithmetic")?;
    let fixtures_dir = fixtures_dir();
    with_fixture(fixtures_dir.clone(), "defaults", |_fixture_dir| {
        // Set up file shims
        shim_path("package.json", fixtures_dir.join("defaults/package.json"));
        shim_path(
            "ubrn.config.yaml",
            fixtures_dir.join("defaults/ubrn-wasm.multifeature.config.yaml"),
        );
        shim_path("rust/shim/Cargo.toml", target_crate.manifest_path());
        shim_path("rust/shim", target_crate.project_root());

        shim_path(
            "rust/crates/wasm-crate/Cargo.toml",
            target_crate.manifest_path(),
        );
        shim_path(
            "libarithmetical.a",
            target_crate.library_path(None, "debug", None),
        );

        run_cli("ubrn build web --config ubrn.config.yaml")?;

        // Assert the expected commands were executed
        assert_commands(&[
            Command::new("cargo")
                .arg("build")
                .arg_pair_suffix("--manifest-path", "fixtures/arithmetic/Cargo.toml")
                .arg_pair("--features", "wasm32_only,some_feature")
                .arg("--no-default-features"),
            // other commands elided.
        ]);

        assert_files(&[
            File::new("rust/crates/wasm-crate/Cargo.toml")
                .contains("[workspace]")
                .contains("uniffi-example-arithmetic = { path = \"../../shim\", features = [\"wasm32_only\", \"some_feature\"], default-features = false }"),
            // other files elided.
        ]);

        Ok(())
    })
}

/// Here we want to put the bindings in a distinct place to the React Native.
/// We're need to add the tsBindings property to the web object in the ubrn.config.yaml.
/// We're also going to use this test to show we're taking the entrypoint from
/// browser property of package.json.
#[test]
fn test_distinct_bindings() -> Result<()> {
    // Still using this target for initial build
    let target_crate = cargo_build("arithmetic")?;
    let fixtures_dir = fixtures_dir();
    with_fixture(fixtures_dir.clone(), "defaults", |_fixture_dir| {
        // Set up file shims
        shim_path(
            "package.json",
            fixtures_dir.join("distinct-bindings/package.json"),
        );
        shim_path(
            "ubrn.config.yaml",
            fixtures_dir.join("distinct-bindings/ubrn.config.yaml"),
        );
        shim_path("rust/shim/Cargo.toml", target_crate.manifest_path());
        shim_path("rust/shim", target_crate.project_root());

        shim_path("rust_modules/wasm/Cargo.toml", target_crate.manifest_path());
        shim_path(
            "libarithmetical.a",
            target_crate.library_path(None, "debug", None),
        );

        shim_path(
            "libarithmetical.a",
            target_crate.library_path(None, "debug", None),
        );

        run_cli("ubrn build web --config ubrn.config.yaml")?;

        assert_files(&[
            // Bindings are set in ubrn.config.yaml, as the web/tsBindings property
            File::new("web-src/bindings/arithmetic.ts"),
            // Entrypoint is set in either: ubrn.config.yaml as web/entrypoint,
            // or in package.json/browser.
            File::new("web-src/index.ts")
                .contains("import * as arithmetic from './bindings/arithmetic';")
                .contains("import initAsync from './bindings/wasm-bindgen/index.js';")
                .contains("import wasmPath from './bindings/wasm-bindgen/index_bg.wasm';"),
        ]);

        Ok(())
    })
}

/// This is similar to the above, but: we're overriding both tsBindings and
/// entrypoint in the ubrn.config.yaml file.
#[test]
fn test_override_entrypoint_in_config_yaml() -> Result<()> {
    // Still using this target for initial build
    let target_crate = cargo_build("arithmetic")?;
    let fixtures_dir = fixtures_dir();
    with_fixture(fixtures_dir.clone(), "defaults", |_fixture_dir| {
        // Set up file shims
        shim_path(
            "package.json",
            fixtures_dir.join("distinct-bindings/package.json"),
        );
        shim_path(
            "ubrn.config.yaml",
            fixtures_dir.join("distinct-bindings/ubrn-with-entrypoint.config.yaml"),
        );
        shim_path("rust/shim/Cargo.toml", target_crate.manifest_path());
        shim_path("rust/shim", target_crate.project_root());

        shim_path("rust_modules/wasm/Cargo.toml", target_crate.manifest_path());
        shim_path(
            "libarithmetical.a",
            target_crate.library_path(None, "debug", None),
        );

        shim_path(
            "libarithmetical.a",
            target_crate.library_path(None, "debug", None),
        );

        run_cli("ubrn build web --config ubrn.config.yaml")?;

        assert_files(&[
            // Bindings are set in ubrn.config.yaml, as the web/tsBindings property
            File::new("bindings/wasm/arithmetic.ts"),
            // Entrypoint is set in either: ubrn.config.yaml as web/entrypoint,
            // or in package.json/browser.
            File::new("entrypoints/web.ts")
                .contains("import * as arithmetic from './../bindings/wasm/arithmetic';")
                .contains("import initAsync from './../bindings/wasm/wasm-bindgen/index.js';")
                .contains("import wasmPath from './../bindings/wasm/wasm-bindgen/index_bg.wasm';"),
        ]);

        Ok(())
    })
}

/// This is similar to the above, but: we're overriding both tsBindings and
/// entrypoint in the ubrn.config.yaml file.
#[test]
fn test_merged_cargo_toml_patch() -> Result<()> {
    // Still using this target for initial build
    let target_crate = cargo_build("arithmetic")?;
    let fixtures_dir = fixtures_dir();
    with_fixture(fixtures_dir.clone(), "merging-toml", |_fixture_dir| {
        // Set up file shims
        shim_path(
            "package.json",
            fixtures_dir.join("merging-toml/package.json"),
        );
        shim_path(
            "ubrn.config.yaml",
            fixtures_dir.join("merging-toml/ubrn.config.yaml"),
        );

        shim_path(
            "Cargo.patch.toml",
            fixtures_dir.join("merging-toml/Cargo.patch.toml"),
        );
        shim_path("rust/shim/Cargo.toml", target_crate.manifest_path());
        shim_path("rust/shim", target_crate.project_root());

        shim_path("rust_modules/wasm/Cargo.toml", target_crate.manifest_path());
        shim_path(
            "libarithmetical.a",
            target_crate.library_path(None, "debug", None),
        );

        shim_path(
            "libarithmetical.a",
            target_crate.library_path(None, "debug", None),
        );

        run_cli("ubrn build web --config ubrn.config.yaml")?;

        assert_files(&[
            // Bindings are set in ubrn.config.yaml, as the web/tsBindings property
            File::new("rust_modules/wasm/Cargo.toml")
                .contains("[dependencies]")
                .contains(
                    "[dependencies.uniffi-example-arithmetic]\nno-default-features = true\npath =",
                )
                .contains("wasm-bindgen = \"PATCHED\"")
                .contains("zzz = \"PATCHED\""),
        ]);

        Ok(())
    })
}

#[test]
fn test_rustflags() -> Result<()> {
    let target_crate = cargo_build("arithmetic")?;
    let fixtures_dir = fixtures_dir();
    with_fixture(fixtures_dir.clone(), "defaults", |_fixture_dir| {
        // Set up file shims
        shim_path("package.json", fixtures_dir.join("defaults/package.json"));
        shim_path(
            "ubrn.config.yaml",
            fixtures_dir.join("defaults/ubrn-wasm.rustflags.config.yaml"),
        );
        shim_path("rust/shim/Cargo.toml", target_crate.manifest_path());
        shim_path("rust/shim", target_crate.project_root());

        shim_path("rust_modules/wasm/Cargo.toml", target_crate.manifest_path());
        shim_path(
            "libarithmetical.a",
            target_crate.library_path(None, "debug", None),
        );

        run_cli("ubrn build web --config ubrn.config.yaml")?;

        // Assert the expected commands were executed, including RUSTFLAGS
        assert_commands(&[
            Command::new("cargo")
                .arg("build")
                .arg_pair_suffix("--manifest-path", "fixtures/arithmetic/Cargo.toml"),
            Command::new("prettier"),
            Command::new("cargo")
                .arg("build")
                .arg_pair_suffix("--manifest-path", "rust_modules/wasm/Cargo.toml")
                .arg_pair("--target", "wasm32-unknown-unknown")
                .env(
                    "RUSTFLAGS",
                    "--cfg web_sys_unstable_apis -C target-feature=+atomics",
                ),
            Command::new("wasm-bindgen")
                .arg_pair("--target", "web")
                .arg("--omit-default-module-path")
                .arg_pair("--out-name", "index")
                .arg_pair_suffix("--out-dir", "src/generated/wasm-bindgen")
                .arg_suffix("wasm32-unknown-unknown/debug/arithmetical.wasm"),
        ]);

        Ok(())
    })
}
