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
    let target = "aarch64-apple-ios";
    let target_crate = cargo_build("arithmetic", target)?;
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
            target_crate.library_path(Some(target), "debug"),
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
    let target = "aarch64-apple-ios"; // Still using this target for initial build
    let target_crate = cargo_build("arithmetic", target)?;
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
            target_crate.library_path(Some(target), "debug"),
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
    let target = "aarch64-apple-ios"; // Still using this target for initial build
    let target_crate = cargo_build("arithmetic", target)?;
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
            target_crate.library_path(Some(target), "debug"),
        );

        run_cli("ubrn build web --config ubrn.config.yaml")?;

        // Assert the expected commands were executed
        assert_commands(&[
            Command::new("cargo")
                .arg("build")
                .arg_pair_suffix("--manifest-path", "fixtures/arithmetic/Cargo.toml")
                .arg_pair("--features", "wasm32_only,some_feature"),
            // other commands elided.
        ]);

        assert_files(&[
            File::new("rust/crates/wasm-crate/Cargo.toml")
                .contains("[workspace]")
                .contains("uniffi-example-arithmetic = { path = \"../../shim\", features = [\"wasm32_only\", \"some_feature\"] }"),
            // other files elided.
        ]);

        Ok(())
    })
}
