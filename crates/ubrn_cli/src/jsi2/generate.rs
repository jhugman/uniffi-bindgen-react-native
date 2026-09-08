/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{ArgGroup, Args, Subcommand};
use ubrn_bindgen::{
    ffi_module_player_lib_resolution::{LibResolution, TripleStyle},
    AbiFlavor, OutputArgs, SourceArgs, SwitchArgs,
};

use crate::{
    commands::{generate::GenerateAllCommand, ConfigArgs},
    config::ProjectConfig,
    Platform,
};

#[derive(Args, Debug)]
pub(crate) struct CmdArg {
    #[clap(subcommand)]
    cmd: Cmd,
}

impl CmdArg {
    pub(crate) fn run(&self) -> Result<()> {
        self.cmd.run()
    }
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Generate just the Typescript bindings for the generic JSI player (Jsi2)
    Bindings(BindingsArgs),

    /// Generate the bindings and every file of an assets-only library
    All(GenerateAllArgs),
}

impl Cmd {
    fn run(&self) -> Result<()> {
        match self {
            Self::Bindings(b) => {
                // Validate before any I/O.
                let resolution = b.resolve_lib_resolution()?;
                let bb = ubrn_bindgen::BindingsArgs::from(b).with_lib_resolution(resolution);
                bb.run(None)?;
                Ok(())
            }
            Self::All(a) => {
                let project: ProjectConfig = a.config.clone().try_into()?;
                GenerateAllCommand::platform_specific(
                    a.lib_file.clone(),
                    project,
                    Platform::Jsi2,
                    false,
                )
                .run()
            }
        }
    }
}

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("lib_resolution")
        .args(["lib_colocated", "lib_absolute", "lib_package_base", "lib_name"])
        .multiple(false)
        .required(true)
))]
pub(crate) struct BindingsArgs {
    #[command(flatten)]
    pub(crate) source: SourceArgs,

    /// By default, bindgen will attempt to format the code with prettier.
    #[clap(long)]
    pub(crate) no_format: bool,

    /// The directory in which to put the generated Typescript.
    #[clap(long)]
    pub(crate) ts_dir: Utf8PathBuf,

    /// Generated bindings call resolveLibPath in colocated mode.
    /// The binary must sit next to the generated `.js` file at runtime.
    #[clap(long = "lib-colocated")]
    pub(crate) lib_colocated: bool,

    /// Generated bindings bake the value of --library as an absolute override path.
    /// Requires --library; the path must be absolute.
    #[clap(long = "lib-absolute", requires = "library_mode")]
    pub(crate) lib_absolute: bool,

    /// Generated bindings resolve the cdylib via `<BASE><triple>` platform npm
    /// packages (e.g. `@scope/foo-aarch64-apple-darwin`) using `require.resolve`.
    /// If BASE ends with an alphanumeric character, a `-` is appended so the
    /// joined name is `BASE-<triple>`; otherwise the trailing character is
    /// used as the literal separator (`@scope/foo/<triple>` for `@scope/foo/`,
    /// `@scope/foo_<triple>` for `@scope/foo_`, etc.). Requires --library so
    /// the crate name can be derived.
    #[clap(
        long = "lib-package-base",
        value_name = "BASE",
        requires = "library_mode"
    )]
    pub(crate) lib_package_base: Option<String>,

    /// The built library's name. Generated bindings pass `{ name: "<NAME>" }`
    /// to `globalThis.uniffi.open` and the host resolves it: the React Native
    /// player looks in the app's native libraries; the Hermes test-runner looks
    /// in $UBRN_JSI_LIB_DIR. Every module generated from that library shares the
    /// name, so its uniffi statics are opened once.
    #[clap(long = "lib-name", value_name = "NAME")]
    pub(crate) lib_name: Option<String>,

    /// With --lib-package-base, emit node-style triples (e.g. `darwin-arm64`,
    /// `linux-x64-gnu`, `win32-x64-msvc`) instead of cargo-style triples.
    /// Has no effect without --lib-package-base; rejected at runtime if used
    /// with --lib-colocated or --lib-absolute.
    #[clap(long = "lib-node-triple")]
    pub(crate) lib_node_triple: bool,
}

impl BindingsArgs {
    fn resolve_lib_resolution(&self) -> Result<LibResolution> {
        if self.lib_node_triple && self.lib_package_base.is_none() {
            anyhow::bail!("--lib-node-triple requires --lib-package-base");
        }
        if let Some(name) = &self.lib_name {
            if name.is_empty() {
                anyhow::bail!("--lib-name requires a non-empty library name");
            }
            return Ok(LibResolution::Name(name.clone()));
        }
        if self.lib_colocated {
            return Ok(LibResolution::Colocated);
        }
        if self.lib_absolute {
            // SourceArgs.source carries --library's path when --library is set.
            let path = self.source.source();
            if !path.is_absolute() {
                anyhow::bail!(
                    "--lib-absolute requires --library to be an absolute path; got: {}",
                    path,
                );
            }
            // Normalize backslashes to forward slashes so the rendered TS string
            // is valid on Windows (Node accepts forward slashes on all platforms).
            // Explicit replace, not path-slash, since path-slash's behavior is
            // host-OS-dependent and we need cross-platform string normalization
            // here (the codegen output is consumed by Node on any OS).
            let normalized = Utf8PathBuf::from(path.as_str().replace('\\', "/"));
            return Ok(LibResolution::Absolute(normalized));
        }
        if let Some(base) = &self.lib_package_base {
            if base.is_empty() {
                anyhow::bail!("--lib-package-base requires a non-empty package base");
            }
            let base = normalize_package_base(base);
            let triple_style = if self.lib_node_triple {
                TripleStyle::Node
            } else {
                TripleStyle::Cargo
            };
            return Ok(LibResolution::Require { base, triple_style });
        }
        // clap's ArgGroup(required = true) on lib_resolution rejects this case at parse.
        unreachable!(
            "clap should have rejected: no --lib-* flag passed (colocated, absolute, package-base, name)"
        )
    }
}

/// Normalize a `--lib-package-base` value into a literal prefix.
///
/// If the last character is alphanumeric (ASCII), append `-` so the runtime
/// produces `BASE-<triple>`. Otherwise leave the value untouched and let the
/// trailing punctuation (`/`, `_`, `-`, …) act as the separator.
fn normalize_package_base(base: &str) -> String {
    match base.chars().next_back() {
        Some(c) if c.is_ascii_alphanumeric() => format!("{base}-"),
        _ => base.to_string(),
    }
}

impl From<&BindingsArgs> for ubrn_bindgen::BindingsArgs {
    fn from(value: &BindingsArgs) -> Self {
        // Jsi2 generates no C++; ts_dir doubles as cpp_dir (unused).
        ubrn_bindgen::BindingsArgs::new(
            SwitchArgs {
                flavor: AbiFlavor::Jsi2,
            },
            value.source.clone(),
            OutputArgs::new(&value.ts_dir, &value.ts_dir, value.no_format),
        )
    }
}

#[derive(Args, Debug)]
pub(crate) struct GenerateAllArgs {
    #[clap(flatten)]
    config: ConfigArgs,

    /// The built library (any slice); bindgen reads uniffi's metadata from it.
    lib_file: Utf8PathBuf,
}

/// The app must list the player itself (autolinking reads only the app's
/// package.json), so the library declares it as a peer; `@ubjs/core` is the
/// runtime the bindings import. Both are added in place only when missing.
pub(crate) fn ensure_package_dependencies(project: &ProjectConfig) -> Result<()> {
    let path = project.project_root().join("package.json");
    if project.exclude_files().is_match("package.json") {
        return Ok(());
    }
    let text = ubrn_common::read_to_string(&path)?;
    let mut json: serde_json::Value = serde_json::from_str(&text)?;
    let range = format!("^{}", project.ubrn_version());
    let root = json
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("{path} is not a JSON object"))?;
    let mut changed = false;
    for (section, name) in [
        ("peerDependencies", "@ubjs/react-native"),
        ("dependencies", "@ubjs/core"),
    ] {
        let deps = root
            .entry(section)
            .or_insert_with(|| serde_json::Value::Object(Default::default()))
            .as_object_mut()
            .ok_or_else(|| anyhow::anyhow!("{path}: {section} is not an object"))?;
        if !deps.contains_key(name) {
            deps.insert(name.to_string(), serde_json::Value::String(range.clone()));
            changed = true;
        }
    }
    if changed {
        // serde_json's preserve_order keeps the file's own key order: an
        // `exports` map resolves differently once its keys are sorted.
        let mut out = serde_json::to_string_pretty(&json)?;
        out.push('\n');
        ubrn_common::write_file(&path, out)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: Cmd,
    }

    fn parse(args: &[&str]) -> Result<TestCli, clap::Error> {
        let mut full = vec!["test", "bindings"];
        full.extend_from_slice(args);
        TestCli::try_parse_from(&full)
    }

    #[test]
    fn lib_colocated_alone_parses() {
        let r = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-colocated",
            "--library",
            "/tmp/foo.dylib",
        ]);
        assert!(r.is_ok(), "got: {:?}", r.err().map(|e| e.to_string()));
    }

    #[test]
    fn lib_absolute_with_library_parses() {
        let r = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-absolute",
            "--library",
            "/tmp/foo.dylib",
        ]);
        assert!(r.is_ok());
    }

    #[test]
    fn lib_absolute_with_relative_library_errors() {
        let cli = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-absolute",
            "--library",
            "rel/foo.dylib",
        ])
        .expect("clap should accept");
        let Cmd::Bindings(b) = cli.cmd else {
            panic!("expected the bindings subcommand")
        };
        assert!(b.resolve_lib_resolution().is_err());
    }

    #[test]
    fn lib_absolute_without_library_errors() {
        // --lib-absolute requires --library (via clap requires = "library_mode")
        let r = parse(&["--ts-dir", "/tmp/ts", "--lib-absolute", "/tmp/foo.dylib"]);
        assert!(r.is_err());
    }

    #[test]
    fn neither_flag_errors() {
        // ArgGroup is required; missing both --lib-colocated and --lib-absolute
        // must fail at parse time.
        let r = parse(&["--ts-dir", "/tmp/ts", "--library", "/tmp/foo.dylib"]);
        assert!(r.is_err());
    }

    #[test]
    fn both_lib_flags_errors() {
        let r = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-colocated",
            "--lib-absolute",
            "--library",
            "/tmp/foo.dylib",
        ]);
        assert!(r.is_err());
    }

    #[test]
    fn lib_package_base_with_library_parses_defaults_to_cargo() {
        let cli = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-package-base",
            "@scope/foo",
            "--library",
            "/tmp/foo.dylib",
        ])
        .expect("clap should accept");
        let Cmd::Bindings(b) = cli.cmd else {
            panic!("expected the bindings subcommand")
        };
        let res = b.resolve_lib_resolution().expect("resolve");
        match res {
            LibResolution::Require { base, triple_style } => {
                assert_eq!(base, "@scope/foo-");
                assert_eq!(triple_style, TripleStyle::Cargo);
            }
            other => panic!("expected Require, got {other:?}"),
        }
    }

    #[test]
    fn lib_package_base_with_node_triple() {
        let cli = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-package-base",
            "@scope/foo",
            "--lib-node-triple",
            "--library",
            "/tmp/foo.dylib",
        ])
        .expect("clap should accept");
        let Cmd::Bindings(b) = cli.cmd else {
            panic!("expected the bindings subcommand")
        };
        let res = b.resolve_lib_resolution().expect("resolve");
        match res {
            LibResolution::Require { base, triple_style } => {
                assert_eq!(base, "@scope/foo-");
                assert_eq!(triple_style, TripleStyle::Node);
            }
            other => panic!("expected Require, got {other:?}"),
        }
    }

    #[test]
    fn lib_node_triple_without_package_base_errors() {
        // --lib-node-triple without --lib-package-base parses (clap accepts) but
        // resolve_lib_resolution rejects it explicitly.
        let cli = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-colocated",
            "--lib-node-triple",
            "--library",
            "/tmp/foo.dylib",
        ])
        .expect("clap should accept");
        let Cmd::Bindings(b) = cli.cmd else {
            panic!("expected the bindings subcommand")
        };
        let err = b.resolve_lib_resolution().expect_err("should reject");
        assert!(err.to_string().contains("--lib-node-triple"), "got: {err}");
    }

    #[test]
    fn lib_package_base_preserves_explicit_separator() {
        // Trailing `/` means the user wants `@scope/foo/<triple>` (subpath layout).
        let cli = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-package-base",
            "@scope/foo/",
            "--library",
            "/tmp/foo.dylib",
        ])
        .expect("clap should accept");
        let Cmd::Bindings(b) = cli.cmd else {
            panic!("expected the bindings subcommand")
        };
        match b.resolve_lib_resolution().expect("resolve") {
            LibResolution::Require { base, .. } => assert_eq!(base, "@scope/foo/"),
            other => panic!("expected Require, got {other:?}"),
        }
    }

    #[test]
    fn lib_package_base_preserves_trailing_hyphen() {
        // Trailing `-` is already a separator; don't double it.
        let cli = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-package-base",
            "@scope/foo-",
            "--library",
            "/tmp/foo.dylib",
        ])
        .expect("clap should accept");
        let Cmd::Bindings(b) = cli.cmd else {
            panic!("expected the bindings subcommand")
        };
        match b.resolve_lib_resolution().expect("resolve") {
            LibResolution::Require { base, .. } => assert_eq!(base, "@scope/foo-"),
            other => panic!("expected Require, got {other:?}"),
        }
    }

    #[test]
    fn lib_package_base_without_library_errors() {
        // --lib-package-base requires --library (via clap requires = "library_mode")
        let r = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-package-base",
            "@scope/foo",
            "/tmp/foo.udl",
        ]);
        assert!(r.is_err());
    }

    #[test]
    fn lib_package_base_conflicts_with_colocated() {
        let r = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-package-base",
            "@scope/foo",
            "--lib-colocated",
            "--library",
            "/tmp/foo.dylib",
        ]);
        assert!(r.is_err());
    }

    #[test]
    fn lib_package_base_conflicts_with_absolute() {
        let r = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-package-base",
            "@scope/foo",
            "--lib-absolute",
            "--library",
            "/tmp/foo.dylib",
        ]);
        assert!(r.is_err());
    }

    #[test]
    fn lib_package_base_empty_errors() {
        let cli = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-package-base",
            "",
            "--library",
            "/tmp/foo.dylib",
        ])
        .expect("clap should accept");
        let Cmd::Bindings(b) = cli.cmd else {
            panic!("expected the bindings subcommand")
        };
        assert!(b.resolve_lib_resolution().is_err());
    }

    #[test]
    fn lib_absolute_normalizes_backslashes_in_path() {
        // Cross-platform string-level normalization (we can't exercise
        // resolve_lib_resolution end-to-end on non-Windows hosts because
        // Utf8Path::is_absolute() rejects "C:\..." on Unix).
        let backslash_path = "C:\\Users\\foo\\lib.dll";
        assert_eq!(backslash_path.replace('\\', "/"), "C:/Users/foo/lib.dll");
    }

    #[test]
    fn normalize_package_base_appends_hyphen_for_alphanumeric_end() {
        assert_eq!(normalize_package_base("@scope/foo"), "@scope/foo-");
        assert_eq!(normalize_package_base("foo"), "foo-");
        assert_eq!(normalize_package_base("foo9"), "foo9-");
    }

    #[test]
    fn normalize_package_base_preserves_existing_separator() {
        assert_eq!(normalize_package_base("@scope/foo-"), "@scope/foo-");
        assert_eq!(normalize_package_base("@scope/foo/"), "@scope/foo/");
        assert_eq!(normalize_package_base("foo_"), "foo_");
        assert_eq!(normalize_package_base("foo."), "foo.");
    }

    #[test]
    fn lib_name_parses_and_resolves() {
        let cli = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-name",
            "my_lib",
            "/tmp/foo.dylib",
        ])
        .expect("clap should accept");
        let Cmd::Bindings(b) = cli.cmd else {
            panic!("expected the bindings subcommand")
        };
        assert!(matches!(
            b.resolve_lib_resolution().expect("resolve"),
            LibResolution::Name(n) if n == "my_lib"
        ));
    }

    #[test]
    fn lib_name_empty_errors() {
        let cli = parse(&["--ts-dir", "/tmp/ts", "--lib-name", "", "/tmp/foo.dylib"])
            .expect("clap should accept");
        let Cmd::Bindings(b) = cli.cmd else {
            panic!("expected the bindings subcommand")
        };
        assert!(b.resolve_lib_resolution().is_err());
    }

    #[test]
    fn lib_name_with_another_lib_flag_errors() {
        let r = parse(&[
            "--ts-dir",
            "/tmp/ts",
            "--lib-name",
            "my_lib",
            "--lib-colocated",
            "/tmp/foo.dylib",
        ]);
        assert!(r.is_err());
    }
}
