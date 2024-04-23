use anyhow::Result;
use camino::Utf8Path;
use clap::Args;
use uniffi_common::{rm_dir, run_cmd};

use crate::{
    bootstrap::BootstrapCmd,
    util::{build_root, cpp_modules, repository_root},
};

#[derive(Debug, Args)]
pub(crate) struct CleanCmd;

impl CleanCmd {
    pub(crate) fn run(&self) -> Result<()> {
        let root = repository_root()?;

        BootstrapCmd::clean_all()?;

        // run this last.
        rm_dir(build_root()?)?;
        rm_dir(cpp_modules()?)?;
        run_cargo_clean(&root)?;
        Ok(())
    }
}

fn run_cargo_clean(dir: &Utf8Path) -> Result<()> {
    run_cmd(
        std::process::Command::new("cargo")
            .arg("clean")
            .current_dir(dir),
    )
}
