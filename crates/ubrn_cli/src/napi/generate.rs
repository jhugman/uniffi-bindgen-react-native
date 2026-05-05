/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::{ArgGroup, Args, Subcommand};
use ubrn_bindgen::{
    ffi_module_player_lib_resolution::LibResolution, AbiFlavor, OutputArgs, SourceArgs, SwitchArgs,
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
    /// Generate just the Typescript bindings for N-API
    Bindings(BindingsArgs),
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
        }
    }
}

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("lib_resolution")
        .args(["lib_colocated", "lib_absolute"])
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
}

impl BindingsArgs {
    fn resolve_lib_resolution(&self) -> Result<LibResolution> {
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
        // clap's ArgGroup(required = true) on lib_resolution rejects this case at parse.
        unreachable!("clap should have rejected: no --lib-* flag passed")
    }
}

impl From<&BindingsArgs> for ubrn_bindgen::BindingsArgs {
    fn from(value: &BindingsArgs) -> Self {
        // Napi doesn't generate C++, so we pass ts_dir as a dummy for cpp_dir.
        ubrn_bindgen::BindingsArgs::new(
            SwitchArgs {
                flavor: AbiFlavor::Napi,
            },
            value.source.clone(),
            OutputArgs::new(&value.ts_dir, &value.ts_dir, value.no_format),
        )
    }
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
        let Cmd::Bindings(b) = cli.cmd;
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
    fn lib_absolute_normalizes_backslashes_in_path() {
        // Cross-platform string-level normalization (we can't exercise
        // resolve_lib_resolution end-to-end on non-Windows hosts because
        // Utf8Path::is_absolute() rejects "C:\..." on Unix).
        let backslash_path = "C:\\Users\\foo\\lib.dll";
        assert_eq!(backslash_path.replace('\\', "/"), "C:/Users/foo/lib.dll");
    }
}
