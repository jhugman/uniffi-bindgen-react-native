/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::{fs, process::Command};

use crate::{
    bootstrap::{Bootstrap, YarnCmd},
    util::{build_root, repository_root},
};

use anyhow::Result;

use camino::{Utf8Path, Utf8PathBuf};
use clap::Args;
use pathdiff::diff_utf8_paths;
use uniffi_common::{find, run_cmd_quietly};

fn typescript_dir() -> Result<Utf8PathBuf> {
    let root = repository_root()?;
    Ok(root.join("typescript"))
}

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

        let file = self.prepare_ts(&file, stem, name)?;
        self.bundle(&file, name)
    }

    pub(crate) fn prepare_ts(
        &self,
        file: &Utf8Path,
        stem: &str,
        bundle_name: &str,
    ) -> Result<Utf8PathBuf> {
        let dir = file.parent().expect("a parent directory for the file");
        let tsconfig = dir.join("tsconfig.json");
        let use_template_tsconfig = !tsconfig.exists();

        let root = repository_root()?;
        if use_template_tsconfig {
            let template_file = typescript_dir()?.join("tsconfig.template.json");
            let root = diff_utf8_paths(root, dir).expect("A path between the file and the repo");
            let contents = fs::read_to_string(template_file)?;
            let contents = contents.replace("{{repository_root}}", root.as_str());
            fs::write(&tsconfig, contents)?;
        }

        let outdir = ts_out_dir()?.join(bundle_name);
        let result = self.compile_ts(&outdir, dir, &tsconfig);
        if use_template_tsconfig {
            fs::remove_file(tsconfig)?;
        }
        result?;
        let entry = find(&outdir, &format!("{stem}.js")).expect("just made this js file");
        Ok(entry)
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

    fn compile_ts(
        &self,
        outdir: &Utf8Path,
        dir: &Utf8Path,
        tsconfig: &Utf8Path,
    ) -> Result<(), anyhow::Error> {
        // tsc.
        // This does the compilation of the ts into js.
        // The tsconfig.json file used to configure it has been copied to the current directory already.
        let tsc = YarnCmd::node_modules()?.join(".bin/tsc");
        let mut cmd = Command::new(tsc);
        run_cmd_quietly(cmd.arg("--outdir").arg(outdir).current_dir(dir))?;

        // tsc-alias:
        // Rewrites absolute paths in to relative paths (configured using the tsconfig.json/paths) so that the
        // metro bundler can include them in the bundle.
        // This is so we can write `import * from 'uniffi-bindgen-react-native` in the generated code, the imported
        // code comes from this package when we test this package, but also come from this when we generate code for
        // client crates.
        let tsc_alias = YarnCmd::node_modules()?.join(".bin/tsc-alias");
        let mut cmd = Command::new(tsc_alias);
        run_cmd_quietly(
            cmd.arg("-p")
                .arg(tsconfig)
                .arg("--outDir")
                .arg(".")
                .current_dir(outdir),
        )?;
        Ok(())
    }
}
