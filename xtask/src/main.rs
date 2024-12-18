/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use clap::{Parser, Subcommand};
use fmt::FmtArgs;
use run::RunCmd;

use crate::{bootstrap::BootstrapCmd, clean::CleanCmd};

mod bootstrap;
mod clean;
mod fmt;
mod run;
mod util;

#[derive(Debug, Parser)]
struct CliArgs {
    #[clap(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Prepare the directory for development
    Bootstrap(BootstrapCmd),
    /// Remove everything as if just git cloned
    Clean(CleanCmd),
    /// Run some Javascript against a Rust library.
    ///
    /// Optionally can compile the Rust.
    Run(RunCmd),

    /// Format all code in the repository
    Fmt(FmtArgs),
}

fn main() -> Result<()> {
    let args = CliArgs::parse();

    match args.cmd {
        Cmd::Bootstrap(c) => c.run(),
        Cmd::Clean(c) => c.run(),
        Cmd::Run(c) => c.run(),
        Cmd::Fmt(c) => c.run(),
    }
}

#[cfg(test)]
mod cli_test {
    use clap::Parser;
    use run::{generate_bindings::GenerateBindingsArg, rust_crate::CrateArg};
    use ubrn_bindgen::AbiFlavor;

    use super::*;
    use crate::run::typescript::EntryArg;

    fn parse(args: &[&str]) -> CliArgs {
        let mut all_args = vec![""];
        all_args.extend_from_slice(args);

        CliArgs::parse_from(all_args)
    }

    #[test]
    fn test_bootstrap_command() {
        assert!(matches!(parse(&["bootstrap"]).cmd, Cmd::Bootstrap(_)));
    }

    #[test]
    fn test_run_command_js_only() {
        let cmd = parse(&["run", "file.ts"]).cmd;

        if let Cmd::Run(RunCmd {
            js_file: EntryArg { file, .. },
            generate_bindings: None,
            crate_: None,
            ..
        }) = &cmd
        {
            assert_eq!(file.as_str(), "file.ts");
        } else {
            panic!("fail")
        }
    }

    #[test]
    fn test_run_command_with_crate() {
        let cmd = parse(&[
            "run",
            "--cpp-dir",
            "cpp-dir/",
            "--ts-dir",
            "ts-dir/",
            "--crate",
            "crate-dir/",
            "file.ts",
        ])
        .cmd;

        let Cmd::Run(RunCmd {
            js_file: EntryArg { file, .. },
            generate_bindings:
                Some(GenerateBindingsArg {
                    ts_dir, abi_dir, ..
                }),
            crate_: Some(CrateArg { crate_dir, .. }),
            switches,
            ..
        }) = &cmd
        else {
            panic!("fail")
        };
        assert_eq!(file.as_str(), "file.ts");
        assert_eq!(ts_dir.as_deref().map(|f| f.as_str()), Some("ts-dir/"));
        assert_eq!(abi_dir.as_deref().map(|f| f.as_str()), Some("cpp-dir/"));
        assert_eq!(crate_dir.as_deref().map(|f| f.as_str()), Some("crate-dir/"));
        assert_eq!(switches.flavor, AbiFlavor::Jsi);
    }

    #[test]
    fn test_run_command_with_wasm() {
        let cmd = parse(&[
            "run",
            "--abi-dir",
            "rust-dir/",
            "--ts-dir",
            "ts-dir/",
            "--crate",
            "crate-dir/",
            "--flavor",
            "wasm",
            "file.ts",
        ])
        .cmd;

        let Cmd::Run(RunCmd {
            js_file: EntryArg { file, .. },
            generate_bindings:
                Some(GenerateBindingsArg {
                    ts_dir, abi_dir, ..
                }),
            crate_: Some(CrateArg { crate_dir, .. }),
            switches,
            ..
        }) = &cmd
        else {
            panic!("fail")
        };

        assert_eq!(file.as_str(), "file.ts");
        assert_eq!(ts_dir.as_deref().map(|f| f.as_str()), Some("ts-dir/"));
        assert_eq!(abi_dir.as_deref().map(|f| f.as_str()), Some("rust-dir/"));
        assert_eq!(crate_dir.as_deref().map(|f| f.as_str()), Some("crate-dir/"));
        assert_eq!(switches.flavor, AbiFlavor::Wasm);
    }

    #[test]
    fn test_run_command_with_jsi() {
        let cmd = parse(&[
            "run",
            "--cpp-dir",
            "cpp-dir/",
            "--ts-dir",
            "ts-dir/",
            "--crate",
            "crate-dir/",
            "--flavor",
            "jsi",
            "file.ts",
        ])
        .cmd;

        let Cmd::Run(RunCmd {
            js_file: EntryArg { file, .. },
            generate_bindings:
                Some(GenerateBindingsArg {
                    ts_dir, abi_dir, ..
                }),
            crate_: Some(CrateArg { crate_dir, .. }),
            switches,
            ..
        }) = &cmd
        else {
            panic!("fail")
        };
        assert_eq!(file.as_str(), "file.ts");
        assert_eq!(ts_dir.as_deref().map(|f| f.as_str()), Some("ts-dir/"));
        assert_eq!(abi_dir.as_deref().map(|f| f.as_str()), Some("cpp-dir/"));
        assert_eq!(crate_dir.as_deref().map(|f| f.as_str()), Some("crate-dir/"));
        assert_eq!(switches.flavor, AbiFlavor::Jsi);
    }
}
