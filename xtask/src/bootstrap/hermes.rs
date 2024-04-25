use std::{fs, process::Command};

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;

use crate::util::{build_root, cpp_modules, rm_dir, run_cmd};

use super::Bootstrap;

#[derive(Debug, Args)]
pub(crate) struct HermesCmd {
    /// Fetch the hermes from this repo
    #[clap(long, default_value = "facebook/hermes")]
    repo: String,

    /// Fetch this branch from the hermes repo
    #[clap(long, short = 'b', default_value = "main")]
    branch: String,
}

impl Default for HermesCmd {
    fn default() -> Self {
        Self {
            repo: "facebook/hermes".to_owned(),
            branch: "main".to_owned(),
        }
    }
}

impl HermesCmd {
    pub fn src_dir() -> Result<Utf8PathBuf> {
        let root = cpp_modules()?;
        Ok(root.join("hermes"))
    }

    pub fn build_dir() -> Result<Utf8PathBuf> {
        let root = build_root()?;
        Ok(root.join("hermes"))
    }

    fn checkout(&self) -> Result<()> {
        let dir = Self::src_dir()?;
        if dir.exists() {
            return Ok(());
        }
        let parent = dir.parent().expect("Hermes directory has no parent");

        fs::create_dir_all(parent)?;
        let repo = format!("https://github.com/{}.git", self.repo);

        let mut cmd = Command::new("git");
        run_cmd(
            cmd.arg("clone")
                .arg("-b")
                .arg(self.branch.as_str())
                .arg(&repo)
                .arg(&dir),
        )?;

        Ok(())
    }
}

impl Bootstrap for HermesCmd {
    fn marker() -> Result<Utf8PathBuf> {
        Self::build_dir()
    }

    fn clean() -> Result<()> {
        rm_dir(&Self::build_dir()?)?;
        rm_dir(&Self::src_dir()?)?;
        Ok(())
    }

    fn prepare(&self) -> Result<()> {
        self.checkout()?;
        let src = Self::src_dir()?;
        let dir = Self::build_dir()?;

        fs::create_dir_all(&dir)?;

        let mut cmd = Command::new("cmake");
        run_cmd(
            cmd.current_dir(&dir)
                .arg("-G")
                .arg("Ninja")
                .arg("-DHERMES_BUILD_APPLE_FRAMEWORK=OFF")
                .arg("-DCMAKE_BUILD_TYPE=Debug")
                .arg(&src),
        )?;

        let mut cmd = Command::new("ninja");
        run_cmd(cmd.current_dir(&dir))?;
        Ok(())
    }
}
