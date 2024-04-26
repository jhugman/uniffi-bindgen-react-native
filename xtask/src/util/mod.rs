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

/// Run the given command, and only output if there is an error.
pub fn run_cmd_quietly(cmd: &mut Command) -> Result<()> {
    cmd.stdin(Stdio::inherit());
    let output = cmd.output().expect("Failed to execute command");

    if !output.status.success() {
        eprintln!("Running {:?}", *cmd);
        eprintln!("{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
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

pub(crate) fn so_extension_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else if cfg!(target_os = "unix") {
        "so"
    } else {
        unimplemented!("Building only on windows, macos and unix supported right now")
    }
}
