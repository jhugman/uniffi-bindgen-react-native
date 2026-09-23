/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! `ubrn build jsi2 {android,ios}` under the recording harness: the commands a
//! build issues and the files it writes, without running cargo-ndk or Xcode.
use anyhow::Result;

use ubrn_cli::test_utils::{cargo_build, fixtures_dir, run_cli};
use ubrn_cli_testing::{assert_commands, assert_files, shim_path, with_fixture, Command, File};

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

#[test]
fn build_ios_wraps_each_platform_as_a_dynamic_framework_bundle() -> Result<()> {
    let target_crate = cargo_build("arithmetic")?;
    let fixtures = fixtures_dir();
    with_fixture(fixtures.clone(), "jsi2", |_| {
        shim_arithmetic(&fixtures, &target_crate);

        run_cli(
            "ubrn build jsi2 ios --config ubrn.config.yaml \
             --targets aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios",
        )?;

        assert_commands(&[
            Command::new("cargo")
                .arg("build")
                .arg_pair_suffix("--manifest-path", "arithmetic/Cargo.toml")
                .arg_pair("--target", "aarch64-apple-ios")
                .env("IPHONEOS_DEPLOYMENT_TARGET", "15.1"),
            Command::new("cargo")
                .arg("build")
                .arg_pair_suffix("--manifest-path", "arithmetic/Cargo.toml")
                .arg_pair("--target", "aarch64-apple-ios-sim")
                .env("IPHONEOS_DEPLOYMENT_TARGET", "15.1"),
            Command::new("cargo")
                .arg("build")
                .arg_pair_suffix("--manifest-path", "arithmetic/Cargo.toml")
                .arg_pair("--target", "x86_64-apple-ios")
                .env("IPHONEOS_DEPLOYMENT_TARGET", "15.1"),
            // Device: one slice, straight into its bundle.
            Command::new("install_name_tool")
                .arg_pair("-id", "@rpath/arithmetical.framework/arithmetical")
                .arg_suffix("jsi2/ios/ios/arithmetical.framework/arithmetical"),
            // Simulator: two slices lipo'd into one bundle.
            Command::new("lipo")
                .arg("-create")
                .arg_suffix("aarch64-apple-ios-sim/debug/libarithmetical.dylib")
                .arg_suffix("x86_64-apple-ios/debug/libarithmetical.dylib")
                .arg_pair_suffix("-output", "jsi2/ios/ios-simulator/libarithmetical.dylib"),
            Command::new("install_name_tool")
                .arg_pair("-id", "@rpath/arithmetical.framework/arithmetical")
                .arg_suffix("jsi2/ios/ios-simulator/arithmetical.framework/arithmetical"),
            Command::new("xcodebuild")
                .arg("-create-xcframework")
                .arg_pair_suffix("-framework", "jsi2/ios/ios/arithmetical.framework")
                .arg_pair_suffix(
                    "-framework",
                    "jsi2/ios/ios-simulator/arithmetical.framework",
                )
                .arg_pair_suffix("-output", "ios/arithmetical.xcframework"),
        ]);

        assert_files(&[
            File::new("jsi2/ios/ios/arithmetical.framework/Info.plist")
                .contains("<key>CFBundleExecutable</key><string>arithmetical</string>")
                .contains(
                    "<key>CFBundleIdentifier</key><string>com.jsi2fixture.arithmetical</string>",
                )
                .contains("<key>CFBundlePackageType</key><string>FMWK</string>")
                .contains("<key>MinimumOSVersion</key><string>15.1</string>")
                .contains("<key>CFBundleShortVersionString</key><string>0.1.0</string>")
                .contains("<string>iPhoneOS</string>"),
            File::new("jsi2/ios/ios-simulator/arithmetical.framework/Info.plist")
                .contains("<key>CFBundleExecutable</key><string>arithmetical</string>")
                .contains("<string>iPhoneSimulator</string>"),
        ]);
        Ok(())
    })
}

#[test]
fn build_ios_and_generate_renders_the_library_from_the_first_slice() -> Result<()> {
    let target_crate = cargo_build("arithmetic")?;
    let fixtures = fixtures_dir();
    with_fixture(fixtures.clone(), "jsi2", |_| {
        shim_arithmetic(&fixtures, &target_crate);
        // The iOS dylibs are never built under recording; bindgen reads the
        // host one through the suffix shim instead.
        shim_path(
            "libarithmetical.dylib",
            target_crate.library_path(None, "debug", Some(true)),
        );

        run_cli("ubrn build jsi2 ios --config ubrn.config.yaml --and-generate --sim-only")?;

        assert_files(&[
            File::new("src/index.tsx").contains("import \"@ubjs/react-native\";"),
            File::new("src/generated/arithmetic-ffi.ts").contains("{ name: \"arithmetical\" }"),
            File::new("Jsi2Fixture.podspec")
                .contains("s.vendored_frameworks = \"ios/arithmetical.xcframework\""),
        ]);
        Ok(())
    })
}
