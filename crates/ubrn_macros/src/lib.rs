/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    bracketed,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Ident, LitStr, Token,
};

struct TestcaseEntry {
    path: LitStr,
    flavors: Vec<Ident>,
}

impl Parse for TestcaseEntry {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let path: LitStr = input.parse()?;
        input.parse::<Token![=>]>()?;
        let content;
        bracketed!(content in input);
        let flavors = Punctuated::<Ident, Token![,]>::parse_terminated(&content)?
            .into_iter()
            .collect();
        Ok(TestcaseEntry { path, flavors })
    }
}

struct TestcaseEntries {
    entries: Vec<TestcaseEntry>,
}

impl Parse for TestcaseEntries {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let entries = Punctuated::<TestcaseEntry, Token![,]>::parse_terminated(input)?
            .into_iter()
            .collect();
        Ok(TestcaseEntries { entries })
    }
}

/// Extract a test name from a file path.
///
/// Handles double extensions like `.test.ts`:
/// - `tests/bindings/test_arithmetic.ts` → `test_arithmetic`
/// - `abort-controller.test.ts` → `abort_controller`
fn sanitize_name(path: &str) -> String {
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(path);
    // Strip all extensions (handles .test.ts, .spec.ts, etc.)
    let stem = file_name.split('.').next().unwrap_or(file_name);
    stem.chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

/// Generate `#[test]` functions for fixture tests, grouped by flavor module.
///
/// ```rust,ignore
/// ubrn_macros::build_foreign_language_testcases! {
///     "tests/bindings/test_arithmetic.ts" => [Jsi, Wasm],
/// }
/// ```
///
/// Expands to one module per flavor, each containing `#[test]` fns:
/// ```rust,ignore
/// mod jsi { #[test] fn test_arithmetic() { ... } }
/// mod wasm { #[test] fn test_arithmetic() { ... } }
/// ```
#[proc_macro]
pub fn build_foreign_language_testcases(input: TokenStream) -> TokenStream {
    build_foreign_language_testcases_impl(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Generate `#[test]` functions for framework TypeScript tests, grouped by flavor module.
///
/// ```rust,ignore
/// ubrn_macros::build_typescript_testcases!(
///     "typescript/tests/*.test.ts" => [Jsi, Wasm]
/// );
/// ```
///
/// Resolves the glob at compile time. Generates one module per flavor,
/// each containing `#[test]` fns for every matched file.
#[proc_macro]
pub fn build_typescript_testcases(input: TokenStream) -> TokenStream {
    build_typescript_testcases_impl(input.into())
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Emit `mod <flavor> { #[test] fn ... }` modules from a map of flavor → test functions.
fn emit_flavor_modules(
    flavor_tests: std::collections::BTreeMap<String, Vec<TokenStream2>>,
) -> TokenStream2 {
    let mut output = TokenStream2::new();
    for (flavor, tests) in &flavor_tests {
        let mod_name = format_ident!("{}", flavor);
        output.extend(quote! {
            mod #mod_name {
                #(#tests)*
            }
        });
    }
    output
}

fn build_foreign_language_testcases_impl(input: TokenStream2) -> syn::Result<TokenStream2> {
    let entries: TestcaseEntries = syn::parse2(input)?;

    // Group tests by flavor so each flavor gets its own module.
    let mut flavor_tests: std::collections::BTreeMap<String, Vec<TokenStream2>> =
        std::collections::BTreeMap::new();

    for entry in &entries.entries {
        let path_str = entry.path.value();
        let sanitized = sanitize_name(&path_str);

        for flavor in &entry.flavors {
            let flavor_lower = flavor.to_string().to_lowercase();
            let test_name = format_ident!("{sanitized}");
            let runner_mod = format_ident!("{}", flavor_lower);

            let test_fn = quote! {
                #[test]
                fn #test_name() {
                    ::ubrn_fixture_testing::#runner_mod::run_test(
                        std::env!("CARGO_PKG_NAME"),
                        concat!(std::env!("CARGO_MANIFEST_DIR"), "/", #path_str),
                        std::env!("CARGO_TARGET_TMPDIR"),
                    );
                }
            };

            flavor_tests.entry(flavor_lower).or_default().push(test_fn);
        }
    }

    Ok(emit_flavor_modules(flavor_tests))
}

fn build_typescript_testcases_impl(input: TokenStream2) -> syn::Result<TokenStream2> {
    let entry: TestcaseEntry = syn::parse2(input)?;
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_err(|e| {
        syn::Error::new(
            entry.path.span(),
            format!("CARGO_MANIFEST_DIR not set: {e}"),
        )
    })?;
    let pattern = format!("{}/{}", manifest_dir, entry.path.value());

    let paths: Vec<_> = glob::glob(&pattern)
        .map_err(|e| syn::Error::new(entry.path.span(), format!("invalid glob: {e}")))?
        .filter_map(|p| p.ok())
        .collect();

    if paths.is_empty() {
        return Err(syn::Error::new(
            entry.path.span(),
            format!("glob matched no files: {}", pattern),
        ));
    }

    let mut flavor_tests: std::collections::BTreeMap<String, Vec<TokenStream2>> =
        std::collections::BTreeMap::new();

    for path in &paths {
        let sanitized = sanitize_name(path.to_str().unwrap_or_default());
        let path_str = path.to_str().unwrap_or_default();

        for flavor in &entry.flavors {
            let flavor_lower = flavor.to_string().to_lowercase();
            let test_name = format_ident!("{sanitized}");
            let flavor_variant = format_ident!("{}", flavor);

            let test_fn = quote! {
                #[test]
                fn #test_name() {
                    ::ubrn_fixture_testing::ts::run_test(
                        #path_str,
                        ::ubrn_fixture_testing::Flavor::#flavor_variant,
                        std::env!("CARGO_TARGET_TMPDIR"),
                    );
                }
            };

            flavor_tests.entry(flavor_lower).or_default().push(test_fn);
        }
    }

    Ok(emit_flavor_modules(flavor_tests))
}
