/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command;

use anyhow::Result;
use clap::{Args, Subcommand};
use ubrn_common::run_cmd;

use crate::{
    bootstrap::{Bootstrap, YarnCmd},
    util::repository_root,
};

#[derive(Debug, Args)]
pub(crate) struct FmtArgs {
    #[clap(subcommand)]
    cmd: Option<LanguageCmd>,
}

pub(crate) trait CodeFormatter {
    fn format_code(&self) -> Result<()>;
}

impl FmtArgs {
    pub(crate) fn run(&self) -> Result<()> {
        self.format_code()
    }
}

impl CodeFormatter for FmtArgs {
    fn format_code(&self) -> Result<()> {
        if let Some(c) = &self.cmd {
            c.format_code()
        } else {
            LanguageCmd::format_all()
        }
    }
}

#[derive(Debug, Subcommand)]
enum LanguageCmd {
    /// Format and optionally clippy
    ///
    /// Options are available to use fmt OR clippy OR both.
    #[clap(aliases = ["rs"])]
    Rust(RustArgs),
    /// Format with prettier
    ///
    /// Use in conjunction with .prettierignore.
    #[clap(aliases = ["ts", "js", "prettier"])]
    Typescript(TypescriptArgs),

    /// Format with clang-format.
    ///
    /// Requires installation of clang-format.
    #[clap(aliases = ["cxx", "c"])]
    Cpp(CppArgs),

    /// Add licence declarations to each source file.
    ///
    /// Configure this with .license-config.yaml
    #[clap(aliases = ["mpl"])]
    Licence(LicenceArgs),
}

impl CodeFormatter for LanguageCmd {
    fn format_code(&self) -> Result<()> {
        match self {
            Self::Rust(c) => c.format_code()?,
            Self::Typescript(c) => c.format_code()?,
            Self::Cpp(c) => c.format_code()?,
            Self::Licence(c) => c.format_code()?,
        }
        Ok(())
    }
}

impl LanguageCmd {
    fn format_all() -> Result<()> {
        // We add to the source code, before formatting.
        LicenceArgs.format_code()?;
        RustArgs::default().format_code()?;
        TypescriptArgs.format_code()?;
        CppArgs.format_code()?;
        Ok(())
    }
}

#[derive(Debug, Default, Args)]
struct RustArgs {
    /// Don't use clippy, just format.
    #[clap(long, conflicts_with_all = ["only_clippy"])]
    pub(crate) no_clippy: bool,

    /// Only use clippy, don't format.
    #[clap(long = "clippy")]
    pub(crate) only_clippy: bool,
}

impl CodeFormatter for RustArgs {
    fn format_code(&self) -> Result<()> {
        let root = repository_root()?;
        let run_fmt = !self.only_clippy;
        let run_clippy = self.only_clippy || !self.no_clippy;
        if run_fmt {
            run_cmd(
                Command::new("cargo")
                    .arg("fmt")
                    .arg("--all")
                    .current_dir(&root),
            )?;
        }

        if run_clippy {
            run_cmd(
                Command::new("cargo")
                    .arg("clippy")
                    .arg("--all")
                    .current_dir(root),
            )?;
        }

        Ok(())
    }
}

#[derive(Debug, Default, Args)]
struct TypescriptArgs;

impl CodeFormatter for TypescriptArgs {
    fn format_code(&self) -> Result<()> {
        YarnCmd.ensure_ready()?;
        let root = repository_root()?;
        if let Some(mut prettier) = ubrn_common::fmt::prettier(root)? {
            run_cmd(&mut prettier)?
        } else {
            unreachable!("Is prettier in package.json dependencies?")
        }
        Ok(())
    }
}

#[derive(Debug, Default, Args)]
struct CppArgs;

impl CodeFormatter for CppArgs {
    fn format_code(&self) -> Result<()> {
        let root = repository_root()?;
        if let Some(mut clang_format) = ubrn_common::fmt::clang_format(root.join("cpp"))? {
            run_cmd(&mut clang_format)?;
        } else {
            eprintln!("clang-format doesn't seem to be installed")
        }
        Ok(())
    }
}

#[derive(Debug, Default, Args)]
struct LicenceArgs;

impl CodeFormatter for LicenceArgs {
    fn format_code(&self) -> Result<()> {
        YarnCmd.ensure_ready()?;
        let root = repository_root()?;
        run_cmd(
            Command::new("./node_modules/.bin/source-licenser")
                .arg(".")
                .arg("--config-file")
                .arg(".licence-config.yaml")
                .current_dir(root),
        )?;
        Ok(())
    }
}
