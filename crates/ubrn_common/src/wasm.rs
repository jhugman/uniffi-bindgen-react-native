/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};

/// The name wasm-ld gives the table that `call_indirect` dispatches through.
const TABLE_EXPORT: &str = "__indirect_function_table";
/// The import namespace wasm-bindgen leaves behind for its CLI to rewrite.
const WBINDGEN_PLACEHOLDER: &str = "__wbindgen_placeholder__";
/// Declares `<stem>_bg.js` for tsc, which will not take an untyped `.js`.
const GLUE_DTS: &str = include_str!("templates/glue.d.ts");
const WBINDGEN_SECTION: &str = "__wasm_bindgen_unstable";
const WBINDGEN_ENV: &str = "UBRN_WASM_BINDGEN";

/// Copy a built `.wasm` into `out_dir` as `<lib_stem>.wasm`, ready for the
/// player.
///
/// A cdylib that pulled in wasm-bindgen-glued dependencies imports
/// `__wbindgen_placeholder__.*`; wasm-bindgen-cli rewrites those against
/// `./<lib_stem>_bg.js`, which the player resolves through `resolveModule`.
/// Renaming its output is safe: the module's own import string lives inside
/// the file. Modules without those imports are copied — wasm-bindgen-cli
/// refuses to run without descriptors.
///
/// `strip_dead_code` runs [`dce_wasm`], whose keep-list is a heuristic.
pub fn stage_wasm(
    built: &Utf8Path,
    out_dir: &Utf8Path,
    lib_stem: &str,
    strip_dead_code: bool,
) -> Result<Utf8PathBuf> {
    let dst = out_dir.join(format!("{lib_stem}.wasm"));
    let _ = std::fs::remove_file(&dst);

    if has_wasm_bindgen_imports(built)? {
        // wasm-bindgen-cli runs its own DCE, and its output depends on a
        // constellation of mangled exports (`_dyn_*`, `__externref_*`) the
        // keep-list below can't safely capture. Trust its result.
        run_wasm_bindgen(built, out_dir, lib_stem)?;
        let bg_wasm = out_dir.join(format!("{lib_stem}_bg.wasm"));
        std::fs::rename(&bg_wasm, &dst).with_context(|| format!("rename {bg_wasm} -> {dst}"))?;

        // Drop wasm-bindgen's entry module, keeping only the `_bg.js` glue.
        // The entry instantiates the wasm for you, which is the player's job:
        // it calls `__wbg_set_wasm` and `__wbindgen_start` itself. Left in
        // place the file is worse than redundant — it is named `<stem>.js`,
        // which shadows the `<stem>.ts` bindings under a bundler resolving
        // `./<stem>.js`, and it imports the `_bg.wasm` renamed above.
        for ext in ["js", "d.ts"] {
            let _ = std::fs::remove_file(out_dir.join(format!("{lib_stem}.{ext}")));
        }

        // The generated `index.ts` imports this; wasm-bindgen writes none.
        crate::write_file(out_dir.join(format!("{lib_stem}_bg.d.ts")), GLUE_DTS)?;
    } else {
        std::fs::copy(built, &dst).with_context(|| format!("copy {built} -> {dst}"))?;
        if strip_dead_code {
            dce_wasm(&dst)?;
        }
    }

    // After whichever rewrite ran above, so neither can drop the export.
    export_growable_table(&dst)?;
    Ok(dst)
}

/// Whether a module imports from wasm-bindgen's placeholder namespace, and so
/// needs `wasm-bindgen-cli` run over it before anything can instantiate it.
///
/// Two callers must agree on this — staging, which runs the rewrite, and the
/// bindgen, which decides whether to import the glue it produces — so it asks
/// the import section rather than scanning for the name, which also appears in
/// the `name` section and in data segments.
pub fn has_wasm_bindgen_imports(wasm_path: &Utf8Path) -> Result<bool> {
    let module = walrus::Module::from_file(wasm_path.as_std_path())
        .map_err(|e| anyhow!("walrus parse {wasm_path}: {e}"))?;
    let found = module
        .imports
        .iter()
        .any(|i| i.module == WBINDGEN_PLACEHOLDER);
    Ok(found)
}

pub fn run_wasm_bindgen(wasm_path: &Utf8Path, out_dir: &Utf8Path, out_name: &str) -> Result<()> {
    let mut cmd = wasm_bindgen_cmd();
    cmd.arg(wasm_path)
        .arg("--target")
        .arg("bundler")
        .arg("--keep-lld-exports")
        .arg("--omit-default-module-path")
        .arg("--out-dir")
        .arg(out_dir)
        .arg("--out-name")
        .arg(out_name);
    crate::run_cmd(&mut cmd).with_context(|| wasm_bindgen_context(wasm_path))
}

pub fn wasm_bindgen_cmd() -> Command {
    let program = std::env::var(WBINDGEN_ENV).unwrap_or_else(|_| "wasm-bindgen".to_string());
    crate::command(program)
}

pub fn wasm_bindgen_context(wasm_path: &Utf8Path) -> String {
    let Ok(Some(version)) = wasm_bindgen_version(wasm_path) else {
        return format!("wasm-bindgen on {wasm_path}");
    };
    format!(
        "wasm-bindgen on {wasm_path}\n\
         It was built against wasm-bindgen {version}, and the rewriter takes \
         only that version:\n    \
         cargo install wasm-bindgen-cli --version {version}\n\
         or a prebuilt binary from \
         https://github.com/wasm-bindgen/wasm-bindgen/releases/tag/{version}. \
         Set {WBINDGEN_ENV} to its path if it cannot go on PATH."
    )
}

pub fn wasm_bindgen_version(wasm_path: &Utf8Path) -> Result<Option<String>> {
    let mut module = walrus::Module::from_file(wasm_path.as_std_path())
        .map_err(|e| anyhow!("walrus parse {wasm_path}: {e}"))?;
    let Some(section) = module.customs.remove_raw(WBINDGEN_SECTION) else {
        return Ok(None);
    };
    Ok(json_string_field(&section.data, "version"))
}

fn json_string_field(data: &[u8], key: &str) -> Option<String> {
    let needle = format!("\"{key}\":\"");
    let at = data
        .windows(needle.len())
        .position(|w| w == needle.as_bytes())?;
    let rest = &data[at + needle.len()..];
    let end = rest.iter().position(|&b| b == b'"')?;
    std::str::from_utf8(&rest[..end]).ok().map(str::to_owned)
}

/// Strip exports the player can't reach from JS, then run walrus' DCE pass.
///
/// wasm-ld keeps every `#[no_mangle] pub extern "C"` export, including the
/// per-type `ffi_*_rustbuffer_reserve` and `uniffi_*_checksum_*` a given crate
/// never uses. Dropping them from the export section lets `gc` reclaim what
/// they held.
///
/// Heuristic keep-list (no bindgen manifest yet):
///   * exports starting with `uniffi_`, `ffi_`, or `__ubrn_`
///   * `memory` and `__indirect_function_table`
///   * `__wbindgen_start` (wasm-bindgen-rewritten cdylibs export this)
///
/// Expect little: `gc` reclaims nothing, so only the export names go, about
/// 0.26% of a release build. The `name` section is a real 27%, but dropping it
/// means `ModuleConfig::generate_name_section(false)` and unreadable stack
/// traces.
pub fn dce_wasm(wasm_path: &Utf8Path) -> Result<()> {
    let mut module = walrus::Module::from_file(wasm_path.as_std_path())
        .map_err(|e| anyhow!("walrus parse {wasm_path}: {e}"))?;

    fn keep(name: &str) -> bool {
        matches!(
            name,
            "memory" | "__indirect_function_table" | "__wbindgen_start"
        ) || name.starts_with("uniffi_")
            || name.starts_with("ffi_")
            || name.starts_with("__ubrn_")
    }

    let to_strip: Vec<_> = module
        .exports
        .iter()
        .filter(|e| !keep(&e.name))
        .map(|e| e.id())
        .collect();
    for id in to_strip {
        module.exports.delete(id);
    }

    walrus::passes::gc::run(&mut module);

    std::fs::write(wasm_path, module.emit_wasm())?;
    Ok(())
}

/// Export a module's function table with no upper bound, so the player can
/// grow it to install callback trampolines.
///
/// wasm-ld does this given `--export-table --growable-table`, but link args
/// must sit on the cdylib's own link step, which a dependency cannot reach —
/// every consumer would need RUSTFLAGS. Rewriting afterwards works whatever
/// built the module.
///
/// A module with no function table has no `call_indirect` sites, so no
/// callbacks to install; it is left alone.
pub fn export_growable_table(wasm_path: &Utf8Path) -> Result<()> {
    let mut module = walrus::Module::from_file(wasm_path.as_std_path())
        .map_err(|e| anyhow!("walrus parse {wasm_path}: {e}"))?;

    let Some(table) = module.tables.main_function_table()? else {
        return Ok(());
    };

    // Dropping the upper bound only widens what the table accepts, so it
    // cannot invalidate the element segments already initialising it.
    module.tables.get_mut(table).maximum = None;

    if !module.exports.iter().any(|e| e.name == TABLE_EXPORT) {
        module.exports.add(TABLE_EXPORT, table);
    }

    std::fs::write(wasm_path, module.emit_wasm())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::json_string_field;

    #[test]
    fn reads_the_version_and_not_the_schema_version() {
        let blob = br#"{"schema_version":"0.2.122","version":"0.2.127"}"#;
        assert_eq!(
            json_string_field(blob, "version").as_deref(),
            Some("0.2.127")
        );
        assert_eq!(
            json_string_field(blob, "schema_version").as_deref(),
            Some("0.2.122")
        );
    }

    #[test]
    fn absent_key_is_none() {
        assert_eq!(json_string_field(br#"{"version":"#, "version"), None);
        assert_eq!(json_string_field(b"", "version"), None);
    }
}
