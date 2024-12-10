/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::Result;
use camino::Utf8Path;
use ubrn_common::{fmt, run_cmd_quietly};

pub(crate) fn format_directory(out_dir: &Utf8Path) -> Result<()> {
    if let Some(mut clang_format) = fmt::clang_format(out_dir, false)? {
        run_cmd_quietly(&mut clang_format)?
    } else {
        eprintln!("Skipping formatting C++. Is clang-format installed?");
    }
    Ok(())
}
