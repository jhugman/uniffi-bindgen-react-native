// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
use anyhow::{ensure, Result};
use camino::Utf8Path;
use ubrn_common::stage_wasm;

fn main() -> Result<()> {
    let built = Utf8Path::new("target/wasm32-unknown-unknown/release/jspi_wasm2_codegen.wasm");
    let out = Utf8Path::new("generated/api");
    std::fs::create_dir_all(out)?;
    let original = std::fs::read(built)?;
    stage_wasm(built, out, "jspi_wasm2_codegen", false)?;
    ensure!(
        std::fs::read(built)? == original,
        "staging modified cargo's artifact"
    );
    ensure!(
        !out.join("jspi_wasm2_codegen.js").exists(),
        "bundler entry shadows TS bindings"
    );
    ensure!(out.join("jspi_wasm2_codegen_bg.js").exists());
    ensure!(out.join("jspi_wasm2_codegen_bg.d.ts").exists());

    Ok(())
}
