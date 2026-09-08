/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! `ubrn generate jsi2 all`: the assets-only library, under the recording
//! harness. Bindgen runs for real against the host cdylib; every file lands in
//! the recorder instead of the fixture directory.
use anyhow::Result;
use camino::Utf8Path;

use ubrn_cli::test_utils::{cargo_build, fixtures_dir, run_cli};
use ubrn_cli_testing::{assert_files, shim_path, with_fixture, File};
use ubrn_common::get_recorded_files;

#[test]
fn generate_all_emits_an_assets_only_library() -> Result<()> {
    let target_crate = cargo_build("arithmetic")?;
    let fixtures = fixtures_dir();
    with_fixture(fixtures.clone(), "jsi2", |fixture_dir| {
        shim_path("package.json", fixtures.join("jsi2/package.json"));
        shim_path("ubrn.config.yaml", fixtures.join("jsi2/ubrn.config.yaml"));
        shim_path("rust/shim/Cargo.toml", target_crate.manifest_path());
        shim_path("rust/shim", target_crate.project_root());
        shim_path(
            "libarithmetical.dylib",
            target_crate.library_path(None, "debug", Some(true)),
        );

        run_cli("ubrn generate jsi2 all --config ubrn.config.yaml libarithmetical.dylib")?;

        assert_files(&[
            // The entrypoint: player first, then the guard, then the modules.
            File::new("src/index.tsx")
                .contains("import \"@ubjs/react-native\";")
                .contains("@ubjs/react-native must be a direct dependency of the app; add it and re-run pod install")
                .contains("export * from './generated/arithmetic';")
                .contains("import * as arithmetic from './generated/arithmetic';")
                .contains("arithmetic.default.initialize();"),
            // The bindings name the library, not a path.
            File::new("src/generated/arithmetic-ffi.ts").contains("{ name: \"arithmetical\" }"),
            File::new("src/generated/arithmetic.ts"),
            // iOS: a vendored dynamic framework and the player as a dependency.
            File::new("Jsi2Fixture.podspec")
                .contains("s.name         = \"Jsi2Fixture\"")
                .contains("s.vendored_frameworks = \"ios/arithmetical.xcframework\"")
                .contains("s.dependency \"UbjsReactNative\""),
            // Android: jniLibs, the player project, and an empty ReactPackage.
            File::new("android/build.gradle")
                .contains("namespace \"com.jsi2fixture\"")
                .does_not_contain("abiFilters")
                .contains("implementation project(\":ubjs_react-native\")")
                .does_not_contain("externalNativeBuild"),
            File::new("android/src/main/AndroidManifest.xml").contains("<manifest"),
            File::new("android/src/main/java/com/jsi2fixture/Jsi2FixturePackage.java")
                .contains("package com.jsi2fixture;")
                .contains("public class Jsi2FixturePackage implements ReactPackage")
                .contains("Collections.emptyList()"),
            // package.json gains the peer and the devDependency; @ubjs/core
            // was already there.
            File::new("package.json")
                .contains("\"@ubjs/react-native\": \"^0.31.0-5\"")
                .contains("\"@ubjs/core\": \"^0.31.0-5\"")
                .contains("\"devDependencies\""),
        ]);

        // The peer alone can't be resolved by the library's own tsc/bob
        // build; it also needs the devDependency copy.
        let package_json = get_recorded_files()
            .into_iter()
            .find(|f| f.path.ends_with("package.json"))
            .expect("package.json was recorded");
        let json: serde_json::Value = serde_json::from_str(&package_json.content)?;
        assert_eq!(json["peerDependencies"]["@ubjs/react-native"], "^0.31.0-5");
        assert_eq!(json["devDependencies"]["@ubjs/react-native"], "^0.31.0-5");

        // Nothing native: no C++, no Kotlin, no CMake, no codegen spec.
        // Matched against the path within the library and the file name, never
        // the absolute path: an ancestor directory may contain anything.
        for file in get_recorded_files() {
            let path = Utf8Path::new(&file.path);
            let rel = path.strip_prefix(&fixture_dir).unwrap_or(path);
            assert!(
                !rel.as_str().contains("cpp/"),
                "assets-only library wrote {rel}"
            );
            let name = path.file_name().expect("a recorded file has a name");
            for forbidden in [".kt", "CMakeLists", "Native", ".mm", ".h"] {
                assert!(
                    !name.contains(forbidden),
                    "assets-only library wrote {}",
                    file.path
                );
            }
        }
        Ok(())
    })
}
