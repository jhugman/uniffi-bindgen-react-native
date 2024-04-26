use std::{fs, process::Command};

use crate::{
    bootstrap::{Bootstrap, YarnCmd},
    util::{build_root, run_cmd_quietly},
};

use anyhow::Result;

use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;

fn build_dir() -> Result<Utf8PathBuf> {
    let root = build_root()?;
    Ok(root.join("js"))
}

fn bundles_dir() -> Result<Utf8PathBuf> {
    let root = build_dir()?;
    Ok(root.join("bundles"))
}

fn ts_out_dir() -> Result<Utf8PathBuf> {
    let root = build_dir()?;
    Ok(root.join("tsc"))
}

#[derive(Debug, Args)]
pub(crate) struct EntryArg {
    /// The Javascript or typescript entry point
    pub(crate) file: Utf8PathBuf,

    /// The name used for the bundle. Defaults to the stem of the file.
    #[clap(long)]
    pub(crate) name: Option<String>,
}

impl EntryArg {
    pub(crate) fn prepare(&self) -> Result<Utf8PathBuf> {
        YarnCmd.ensure_ready()?;
        let file = self.file.canonicalize_utf8()?;
        let stem = file.file_stem().expect("a filename with an extension");
        let name = self.name.as_deref().unwrap_or(stem);

        let file = self.compile_ts(&file, stem, name)?;
        self.bundle(&file, name)
    }

    pub(crate) fn compile_ts(
        &self,
        file: &Utf8Path,
        stem: &str,
        bundle_name: &str,
    ) -> Result<Utf8PathBuf> {
        let tsc = YarnCmd::node_modules()?.join(".bin/tsc");
        let outdir = ts_out_dir()?.join(bundle_name);

        let mut cmd = Command::new(tsc);
        run_cmd_quietly(
            cmd.arg("--esModuleInterop")
                .arg("--allowJs")
                .arg("--checkJs")
                .arg("--outdir")
                .arg(&outdir)
                .arg(file),
        )?;
        Ok(outdir.join(format!("{stem}.js")))
    }

    pub(crate) fn bundle(&self, file: &Utf8Path, bundle_name: &str) -> Result<Utf8PathBuf> {
        let dir = bundles_dir()?;
        fs::create_dir_all(&dir)?;

        let bundle_path = dir.join(format!("{bundle_name}.bundle.js"));

        let metro = YarnCmd::node_modules()?.join(".bin/metro");
        let mut cmd = Command::new(metro);
        run_cmd_quietly(
            cmd.arg("build")
                .arg("--minify")
                .arg("false")
                .arg("--out")
                .arg(&bundle_path)
                .arg(file),
        )?;

        Ok(bundle_path)
    }
}
