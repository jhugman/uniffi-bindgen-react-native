/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! `ubrn build jsi2 {android,ios}` under the recording harness: the commands a
//! build issues and the files it writes, without running cargo-ndk or Xcode.
use anyhow::Result;

use ubrn_cli::test_utils::{cargo_build, fixtures_dir, run_cli};
use ubrn_cli_testing::{assert_commands, shim_path, with_fixture, Command};

fn shim_arithmetic(fixtures: &camino::Utf8Path, target_crate: &ubrn_common::CrateMetadata) {
    shim_path("package.json", fixtures.join("jsi2/package.json"));
    shim_path("ubrn.config.yaml", fixtures.join("jsi2/ubrn.config.yaml"));
    shim_path("rust/shim/Cargo.toml", target_crate.manifest_path());
    shim_path("rust/shim", target_crate.project_root());
}

#[test]
fn build_android_cross_compiles_the_shipped_abis_as_shared_libraries() -> Result<()> {
    let target_crate = cargo_build("arithmetic")?;
    let fixtures = fixtures_dir();
    with_fixture(fixtures.clone(), "jsi2", |_| {
        shim_arithmetic(&fixtures, &target_crate);

        run_cli("ubrn build jsi2 android --config ubrn.config.yaml")?;

        // 64-bit only by default, each 16 KB-aligned; the crate is the
        // arithmetic example, whose cdylib is `arithmetical`.
        assert_commands(&[
            Command::new("cargo")
                .arg("ndk")
                .arg_pair_suffix("--manifest-path", "arithmetic/Cargo.toml")
                .arg_pair("--target", "arm64-v8a")
                .arg_pair("--platform", "23")
                .arg("--")
                .arg("build")
                .env(
                    "CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS",
                    "-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-soname,libarithmetical.so",
                ),
            Command::new("cargo")
                .arg("ndk")
                .arg_pair_suffix("--manifest-path", "arithmetic/Cargo.toml")
                .arg_pair("--target", "x86_64")
                .arg_pair("--platform", "23")
                .arg("--")
                .arg("build")
                .env(
                    "CARGO_TARGET_X86_64_LINUX_ANDROID_RUSTFLAGS",
                    "-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-soname,libarithmetical.so",
                ),
        ]);
        Ok(())
    })
}

#[test]
fn build_android_honours_targets_override() -> Result<()> {
    let target_crate = cargo_build("arithmetic")?;
    let fixtures = fixtures_dir();
    with_fixture(fixtures.clone(), "jsi2", |_| {
        shim_arithmetic(&fixtures, &target_crate);

        run_cli("ubrn build jsi2 android --config ubrn.config.yaml --targets armeabi-v7a")?;

        assert_commands(&[Command::new("cargo")
            .arg("ndk")
            .arg_pair_suffix("--manifest-path", "arithmetic/Cargo.toml")
            .arg_pair("--target", "armeabi-v7a")
            .arg_pair("--platform", "23")
            .arg("--")
            .arg("build")]);
        Ok(())
    })
}
