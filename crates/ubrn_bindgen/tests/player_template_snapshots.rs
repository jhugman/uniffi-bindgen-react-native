/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use expect_test::expect;
use ubrn_bindgen::__player_template_test::{render_minimal_for_test, LibResolution, TripleStyle};

fn extract_getter_block(rendered: &str) -> String {
    let start = rendered
        .find("let _nativeModule")
        .expect("could not find getter start in rendered template");
    let end = rendered
        .find("export default getter;")
        .expect("could not find getter end in rendered template");
    rendered[start..end].trim().to_string() + "\nexport default getter;"
}

#[test]
fn template_colocated() {
    let rendered = render_minimal_for_test(LibResolution::Colocated, "my_crate");
    expect![[r#"
        let _nativeModule: NativeModuleInterface | undefined;
        const getter: () => NativeModuleInterface = () => {
          if (!_nativeModule) {
            const libPath = resolveLibPath({
              crateName: "my_crate",
              callerUrl: import.meta.url,
            });
            const mod_ = UniffiNativeModule.open(libPath);
            _nativeModule = mod_.register(DEFINITIONS) as unknown as NativeModuleInterface;
          }
          return _nativeModule;
        };
        export default getter;"#]]
    .assert_eq(&extract_getter_block(&rendered));
}

#[test]
fn template_absolute() {
    let rendered = render_minimal_for_test(
        LibResolution::Absolute(camino::Utf8PathBuf::from("/abs/lib.so")),
        "my_crate",
    );
    expect![[r#"
        let _nativeModule: NativeModuleInterface | undefined;
        const getter: () => NativeModuleInterface = () => {
          if (!_nativeModule) {
            const libPath = resolveLibPath({
              crateName: "my_crate",
              callerUrl: import.meta.url,
              override: "/abs/lib.so",
            });
            const mod_ = UniffiNativeModule.open(libPath);
            _nativeModule = mod_.register(DEFINITIONS) as unknown as NativeModuleInterface;
          }
          return _nativeModule;
        };
        export default getter;"#]]
    .assert_eq(&extract_getter_block(&rendered));
}

#[test]
fn template_require_cargo() {
    let rendered = render_minimal_for_test(
        LibResolution::Require {
            base: "@scope/foo-".to_string(),
            triple_style: TripleStyle::Cargo,
        },
        "my_crate",
    );
    expect![[r#"
        let _nativeModule: NativeModuleInterface | undefined;
        const getter: () => NativeModuleInterface = () => {
          if (!_nativeModule) {
            const libPath = resolveLibPath({
              crateName: "my_crate",
              callerUrl: import.meta.url,
              npmPackageBase: "@scope/foo-",
              tripleStyle: "cargo",
            });
            const mod_ = UniffiNativeModule.open(libPath);
            _nativeModule = mod_.register(DEFINITIONS) as unknown as NativeModuleInterface;
          }
          return _nativeModule;
        };
        export default getter;"#]]
    .assert_eq(&extract_getter_block(&rendered));
}

#[test]
fn template_require_node() {
    let rendered = render_minimal_for_test(
        LibResolution::Require {
            base: "@scope/foo-".to_string(),
            triple_style: TripleStyle::Node,
        },
        "my_crate",
    );
    expect![[r#"
        let _nativeModule: NativeModuleInterface | undefined;
        const getter: () => NativeModuleInterface = () => {
          if (!_nativeModule) {
            const libPath = resolveLibPath({
              crateName: "my_crate",
              callerUrl: import.meta.url,
              npmPackageBase: "@scope/foo-",
              tripleStyle: "node",
            });
            const mod_ = UniffiNativeModule.open(libPath);
            _nativeModule = mod_.register(DEFINITIONS) as unknown as NativeModuleInterface;
          }
          return _nativeModule;
        };
        export default getter;"#]]
    .assert_eq(&extract_getter_block(&rendered));
}

#[test]
fn template_require_preserves_non_hyphen_separator() {
    // Subpath layout: `@scope/foo/<triple>` — base ends with `/` so no `-` is added.
    let rendered = render_minimal_for_test(
        LibResolution::Require {
            base: "@scope/foo/".to_string(),
            triple_style: TripleStyle::Cargo,
        },
        "my_crate",
    );
    let block = extract_getter_block(&rendered);
    assert!(
        block.contains(r#"npmPackageBase: "@scope/foo/""#),
        "expected subpath base verbatim, got:\n{block}"
    );
}

#[test]
fn template_absolute_with_windows_path_uses_forward_slashes() {
    // Even on darwin, the snapshot test exercises that the template renders
    // a path with forward slashes verbatim — which is what we'll feed it from
    // resolve_lib_resolution after backslash normalization.
    let rendered = render_minimal_for_test(
        LibResolution::Absolute(camino::Utf8PathBuf::from("C:/Users/foo/lib.dll")),
        "my_crate",
    );
    let block = extract_getter_block(&rendered);
    assert!(
        block.contains(r#"override: "C:/Users/foo/lib.dll""#),
        "expected forward-slash path in rendered template, got:\n{block}"
    );
    // No backslash should appear in the rendered string literal.
    assert!(!block.contains('\\'), "backslash leaked: {block}");
}
