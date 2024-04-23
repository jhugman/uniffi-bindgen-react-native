use anyhow::Result;
use std::process::{Command, Stdio};

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
        anyhow::bail!("FAILED");
    }

    Ok(())
}
