/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8Path;
use std::process::Command;
use which::which;

use crate::{file_paths, resolve};

pub fn clang_format<P: AsRef<Utf8Path>>(path: P, check_only: bool) -> Result<Option<Command>> {
    if which("clang-format").is_err() {
        use crate::testing::{is_recording_enabled, record_command};
        if is_recording_enabled() {
            record_command(&Command::new("clang-format"));
        }
        return Ok(None);
    }

    let path = path.as_ref();
    let mut cmd = Command::new("clang-format");
    if check_only {
        cmd.arg("--dry-run").arg("--Werror");
    } else {
        cmd.arg("-i");
    }
    cmd.arg("--style=file")
        .arg("--fallback-style=LLVM")
        .args(file_paths(&format!("{path}/**/*.[ch]"))?)
        .args(file_paths(&format!("{path}/**/*.[ch]pp"))?)
        .current_dir(path);
    Ok(Some(cmd))
}

pub fn prettier<P: AsRef<Utf8Path>>(out_dir: P, check_only: bool) -> Result<Option<Command>> {
    let prettier = resolve(&out_dir, "node_modules/.bin/prettier")?;
    Ok(if let Some(prettier) = prettier {
        let mut cmd = Command::new(prettier);
        if check_only {
            cmd.arg("--check");
        } else {
            cmd.arg("--write");
        }
        cmd.arg(".").current_dir(out_dir.as_ref());
        Some(cmd)
    } else {
        use crate::testing::{is_recording_enabled, record_command};
        if is_recording_enabled() {
            record_command(&Command::new("prettier"));
        }
        None
    })
}
