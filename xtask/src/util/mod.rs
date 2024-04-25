use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use std::{
    env, fs,
    process::{Command, Stdio},
};

pub fn repository_root() -> Result<Utf8PathBuf> {
    let dir = env::var("CARGO_MANIFEST_DIR").context("failed to get manifest dir")?;
    Ok(Utf8Path::new(&*dir).parent().unwrap().to_path_buf())
}

pub fn build_root() -> Result<Utf8PathBuf> {
    let dir = repository_root()?;
    Ok(dir.join("build"))
}

pub fn cpp_modules() -> Result<Utf8PathBuf> {
    let dir = repository_root()?;
    Ok(dir.join("cpp_modules"))
}

pub fn run_cmd(cmd: &mut Command) -> Result<()> {
    eprintln!("Running {:?}", *cmd);
    cmd.stdin(Stdio::inherit());

    let status = cmd.status()?;

    if !status.success() {
        anyhow::bail!("Failed to run command");
    }

    Ok(())
}

pub(crate) fn rm_dir(dir: &Utf8Path) -> Result<()> {
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
}
