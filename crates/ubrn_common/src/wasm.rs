/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};

/// The name wasm-ld gives the table that `call_indirect` dispatches through.
const TABLE_EXPORT: &str = "__indirect_function_table";
/// The import namespace wasm-bindgen leaves behind for its CLI to rewrite.
const WBINDGEN_PLACEHOLDER: &str = "__wbindgen_placeholder__";

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

/// In-process equivalent of `wasm-bindgen <wasm> --target bundler
/// --keep-lld-exports --out-dir <out_dir> --out-name <out_name>`.
pub fn run_wasm_bindgen(wasm_path: &Utf8Path, out_dir: &Utf8Path, out_name: &str) -> Result<()> {
    use wasm_bindgen_cli_support::Bindgen;
    Bindgen::new()
        .input_path(wasm_path.as_std_path())
        .bundler(true)
        .map_err(|e| anyhow!("wasm-bindgen: bundler target unavailable: {e}"))?
        .keep_lld_exports(true)
        .omit_default_module_path(true)
        .out_name(out_name)
        .generate(out_dir.as_std_path())
        .map_err(|e| anyhow!("wasm-bindgen on {wasm_path}: {e}"))
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
