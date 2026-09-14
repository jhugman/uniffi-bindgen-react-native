/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! Extract UniFFI metadata directly from a `wasm32-unknown-unknown` cdylib.
//!
//! When Rust compiles a UniFFI crate to wasm, each `UNIFFI_META_*` symbol
//! becomes an exported i32 global whose value is a linear-memory address
//! pointing into a data segment. The bytes at that address use the same
//! self-describing binary format as native (ELF/Mach-O/PE) builds, so we
//! feed them directly into [`uniffi_meta::read_metadata`].
//!
//! This lets the bindings generator skip the second native cargo build whose
//! sole purpose was to populate a dylib symbol table.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use uniffi_meta::Metadata;
use wasmparser::{
    Export, ExternalKind, GlobalType, Imports, Operator, Parser, Payload, TypeRef, ValType,
};

/// Returns true if `bytes` looks like a WebAssembly module (magic + version).
pub(crate) fn looks_like_wasm(bytes: &[u8]) -> bool {
    bytes.len() >= 8 && &bytes[..4] == b"\0asm"
}

/// Read a wasm cdylib from disk and extract every `UNIFFI_META_*` blob.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn extract_from_wasm(path: &Path) -> Result<Vec<Metadata>> {
    let wasm_bytes =
        std::fs::read(path).with_context(|| format!("failed to read WASM: {}", path.display()))?;
    extract_from_wasm_bytes(&wasm_bytes)
}

/// Same as [`extract_from_wasm`], but takes pre-loaded bytes.
pub(crate) fn extract_from_wasm_bytes(wasm_bytes: &[u8]) -> Result<Vec<Metadata>> {
    let blobs = read_meta_blobs(wasm_bytes)?;
    blobs
        .into_iter()
        .map(|(name, bytes)| {
            uniffi_meta::read_metadata(bytes)
                .with_context(|| format!("failed to parse metadata for '{name}'"))
        })
        .collect()
}

/// Locate every `UNIFFI_META_*` exported global and return `(symbol, &bytes)`
/// pairs pointing into the module's data segments. Split out so tests can
/// inspect raw bytes without a fully-formed `Metadata`.
fn read_meta_blobs(wasm_bytes: &[u8]) -> Result<Vec<(String, &[u8])>> {
    let mut imported_global_count: u32 = 0;
    let mut globals: Vec<i32> = Vec::new();
    let mut meta_exports: BTreeMap<String, u32> = BTreeMap::new();
    let mut data_segments: Vec<(u32, &[u8])> = Vec::new();

    for payload in Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload.context("failed to parse WASM payload")?;
        match payload {
            Payload::ImportSection(reader) => {
                for group in reader {
                    let group = group.context("failed to parse WASM import")?;
                    imported_global_count += count_imported_globals(group)?;
                }
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    let global = global.context("failed to parse WASM global")?;
                    let value = eval_i32_const_expr(&global.ty, &global.init_expr)?;
                    globals.push(value);
                }
            }
            Payload::ExportSection(reader) => {
                for export in reader {
                    let Export { name, kind, index } =
                        export.context("failed to parse WASM export")?;
                    if kind == ExternalKind::Global && is_uniffi_meta_symbol(name) {
                        meta_exports.insert(name.to_string(), index);
                    }
                }
            }
            Payload::DataSection(reader) => {
                for data in reader {
                    let data = data.context("failed to parse WASM data segment")?;
                    if let wasmparser::DataKind::Active {
                        memory_index: 0,
                        offset_expr,
                    } = &data.kind
                    {
                        let base = eval_i32_init_expr(offset_expr)?;
                        data_segments.push((base as u32, data.data));
                    }
                }
            }
            _ => {}
        }
    }

    if meta_exports.is_empty() {
        bail!("no UNIFFI_META_* exports found in WASM file - is this a UniFFI crate?");
    }

    let mut out = Vec::with_capacity(meta_exports.len());
    for (name, global_index) in meta_exports {
        let local_index = global_index
            .checked_sub(imported_global_count)
            .ok_or_else(|| {
                anyhow!(
                    "global index {global_index} refers to an imported global \
                 (not a local data pointer) for export '{name}'"
                )
            })?;
        let addr = *globals.get(local_index as usize).ok_or_else(|| {
            anyhow!("global index {global_index} out of range for export '{name}'")
        })? as u32;

        let bytes = read_from_data_segments(&data_segments, addr).with_context(|| {
            format!("failed to read metadata bytes for '{name}' at address {addr:#x}")
        })?;
        out.push((name, bytes));
    }

    Ok(out)
}

/// Count the globals an import group contributes. `wasmparser::Imports` has
/// three variants — `Single` and two compact encodings — all of which can
/// carry them.
fn count_imported_globals(group: Imports<'_>) -> Result<u32> {
    Ok(match group {
        Imports::Single(_, import) => u32::from(matches!(import.ty, TypeRef::Global(_))),
        Imports::Compact1 { items, .. } => {
            let mut n = 0u32;
            for item in items {
                let item = item.context("failed to parse compact import item")?;
                if matches!(item.ty, TypeRef::Global(_)) {
                    n += 1;
                }
            }
            n
        }
        Imports::Compact2 { ty, names, .. } => {
            if matches!(ty, TypeRef::Global(_)) {
                let mut n = 0u32;
                for name in names {
                    let _ = name.context("failed to parse compact import name")?;
                    n += 1;
                }
                n
            } else {
                0
            }
        }
    })
}

fn eval_i32_const_expr(ty: &GlobalType, init_expr: &wasmparser::ConstExpr) -> Result<i32> {
    if ty.content_type != ValType::I32 {
        bail!("expected i32 global, got {:?}", ty.content_type);
    }
    eval_i32_init_expr(init_expr)
}

fn eval_i32_init_expr(expr: &wasmparser::ConstExpr) -> Result<i32> {
    let mut reader = expr.get_operators_reader();
    match reader.read()? {
        Operator::I32Const { value } => Ok(value),
        other => bail!("expected I32Const in const expr, got: {other:?}"),
    }
}

fn is_uniffi_meta_symbol(name: &str) -> bool {
    let name = name.strip_prefix('_').unwrap_or(name);
    name.starts_with("UNIFFI_META")
}

fn read_from_data_segments<'a>(segments: &[(u32, &'a [u8])], addr: u32) -> Result<&'a [u8]> {
    for (base, data) in segments {
        let end = base.saturating_add(data.len() as u32);
        if addr >= *base && addr < end {
            let offset = (addr - base) as usize;
            return Ok(&data[offset..]);
        }
    }
    bail!("address {addr:#x} not found in any active data segment");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hand-rolled minimal wasm module:
    ///   - 1 memory (1 page)
    ///   - 1 i32 global, init = i32.const 1024
    ///   - exports the global as `UNIFFI_META_NS_demo`
    ///   - 1 active data segment at offset 1024 containing the bytes 1..=8
    ///
    /// We assert `read_meta_blobs` finds the export and slices out the right
    /// bytes; we deliberately don't pipe through `read_metadata` because the
    /// payload isn't a real metadata blob.
    fn build_minimal_wasm() -> Vec<u8> {
        // Sections are LEB128-prefixed payloads. For tiny known sizes we can
        // hand-encode lengths as single bytes (all values < 0x80).
        let mut out = Vec::new();
        // magic + version
        out.extend_from_slice(b"\0asm");
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);

        // ---- memory section (id=5): 1 memory, limits min=1, no max
        let mem_payload = vec![0x01, 0x00, 0x01];
        push_section(&mut out, 5, &mem_payload);

        // ---- global section (id=6): 1 global, type=i32 (0x7f) mut=0, init expr i32.const 1024 end
        // 1024 in signed-LEB128 = 0x80 0x08
        let global_payload = vec![
            0x01, // count
            0x7f, 0x00, // valtype i32, immutable
            0x41, 0x80, 0x08, 0x0b, // i32.const 1024 ; end
        ];
        push_section(&mut out, 6, &global_payload);

        // ---- export section (id=7): 1 export, name "UNIFFI_META_NS_demo", kind=global(2), index=0
        let name = b"UNIFFI_META_NS_demo";
        let mut export_payload = vec![0x01, name.len() as u8];
        export_payload.extend_from_slice(name);
        export_payload.extend_from_slice(&[0x03, 0x00]); // kind global (0x03), index 0
        push_section(&mut out, 7, &export_payload);

        // ---- data section (id=11): 1 active segment, memidx=0, offset=i32.const 1024 end, 8 bytes
        let mut data_payload = vec![
            0x01, // segment count
            0x00, // active in memory 0
            0x41, 0x80, 0x08, 0x0b, // i32.const 1024 ; end
            0x08, // 8 bytes
        ];
        data_payload.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        push_section(&mut out, 11, &data_payload);

        out
    }

    fn push_section(out: &mut Vec<u8>, id: u8, payload: &[u8]) {
        out.push(id);
        // payload length as LEB128; for our small payloads single-byte is enough
        assert!(payload.len() < 0x80, "payload too big for single-byte LEB");
        out.push(payload.len() as u8);
        out.extend_from_slice(payload);
    }

    #[test]
    fn extracts_meta_blob_from_minimal_wasm() {
        let wasm = build_minimal_wasm();
        assert!(looks_like_wasm(&wasm));
        let blobs = read_meta_blobs(&wasm).expect("should parse minimal wasm");
        assert_eq!(blobs.len(), 1);
        let (name, bytes) = &blobs[0];
        assert_eq!(name, "UNIFFI_META_NS_demo");
        // The data segment is exactly 8 bytes and the address points at its base,
        // so the returned slice begins with our sentinel and is at least 8 bytes.
        assert!(bytes.len() >= 8);
        assert_eq!(&bytes[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn rejects_wasm_with_no_meta_exports() {
        // A genuinely empty wasm module - just the header.
        let wasm: Vec<u8> = b"\0asm\x01\x00\x00\x00".to_vec();
        let err = read_meta_blobs(&wasm).expect_err("should fail without meta exports");
        assert!(err.to_string().contains("no UNIFFI_META_*"));
    }

    /// Extract from *every* fixture wasm the wasm2 harness has built. Between
    /// them the fixtures cover metadata shapes no single crate does — async,
    /// callback interfaces, trait methods, external types — so this is what
    /// shows wasm extraction can stand in for the native build.
    ///
    /// `#[ignore]` because the inputs live under `target/tmp/` and only exist
    /// once the wasm2 fixture suite has run. To regenerate: run
    /// `cargo test -- wasm2::`, then `cargo test -p ubrn_bindgen --features
    /// wasm -- --ignored extract_from_real_fixture_wasm`.
    #[test]
    #[ignore]
    fn extract_from_real_fixture_wasm() {
        // Resolve the workspace root from CARGO_MANIFEST_DIR so the test
        // doesn't depend on the current working directory.
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("crate is at <workspace>/crates/ubrn_bindgen")
            .to_path_buf();
        // `ubrn_fixture_testing::wasm2::compile_wasm32` builds `--release`.
        let dir = workspace_root
            .join("target/tmp/ubrn-tests-shared/wasm2-target/wasm32-unknown-unknown/release");
        let mut wasms: Vec<_> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| {
                panic!(
                    "no fixture wasms at {}: {e}. Build them first.",
                    dir.display()
                )
            })
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "wasm"))
            // A lib artifact is named after `[lib] name`, which cannot contain
            // a hyphen; a bin artifact keeps its name verbatim. So a hyphen
            // means an executable, which carries no UniFFI metadata.
            .filter(|p| {
                !p.file_stem()
                    .is_some_and(|s| s.to_string_lossy().contains('-'))
            })
            .collect();
        wasms.sort();
        assert!(!wasms.is_empty(), "no .wasm files in {}", dir.display());

        let mut failures = Vec::new();
        for path in &wasms {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            match extract_from_wasm(path) {
                Err(e) => failures.push(format!("{name}: {e:#}")),
                Ok(items) if items.is_empty() => failures.push(format!("{name}: no metadata")),
                Ok(items) if !items.iter().any(|m| matches!(m, Metadata::Namespace(_))) => {
                    failures.push(format!("{name}: no Namespace metadata"))
                }
                Ok(_) => {}
            }
        }
        assert!(
            failures.is_empty(),
            "{} of {} fixture wasms failed extraction:\n  {}",
            failures.len(),
            wasms.len(),
            failures.join("\n  ")
        );
        eprintln!("extracted metadata from {} fixture wasms", wasms.len());
    }
}
