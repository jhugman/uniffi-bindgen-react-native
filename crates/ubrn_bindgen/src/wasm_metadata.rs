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

use anyhow::{anyhow, bail, ensure, Context, Result};
use uniffi_meta::Metadata;
use wasmparser::{
    BlockType, Export, ExternalKind, FunctionBody, GlobalType, Imports, MemoryType, Operator,
    Parser, Payload, TypeRef, ValType,
};

// A Rust-generated start function only initializes static data. Refuse large or
// unfamiliar initializers instead of guessing at the module's memory contents.
const MAX_START_BODY_BYTES: usize = 64 * 1024;

struct MemoryWrite<'a> {
    base: u32,
    len: u32,
    bytes: Option<&'a [u8]>,
}

impl<'a> MemoryWrite<'a> {
    fn data(base: u32, bytes: &'a [u8]) -> Result<Self> {
        let len = u32::try_from(bytes.len()).context("WASM data segment is too large")?;
        Self::new(base, len, Some(bytes))
    }

    fn new(base: u32, len: u32, bytes: Option<&'a [u8]>) -> Result<Self> {
        base.checked_add(len)
            .context("WASM memory write exceeds the 32-bit address space")?;
        Ok(Self { base, len, bytes })
    }

    fn end(&self) -> u32 {
        self.base + self.len // checked in new
    }
}

enum StartupWrite {
    Init {
        data_index: u32,
        destination: u32,
        source: u32,
        length: u32,
    },
    Overwrite {
        destination: u32,
        length: u32,
    },
}

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
    let mut imported_function_count: u32 = 0;
    let mut imported_global_count: u32 = 0;
    let mut initial_memory_bytes = None;
    let mut globals: Vec<(i32, bool)> = Vec::new();
    let mut meta_exports: BTreeMap<String, u32> = BTreeMap::new();
    let mut writes: Vec<MemoryWrite<'_>> = Vec::new();
    let mut data_segments: Vec<Option<&[u8]>> = Vec::new();
    let mut has_passive_data = false;
    let mut start_function = None;
    let mut start_body = None;
    let mut local_function_index = 0u32;

    for payload in Parser::new(0).parse_all(wasm_bytes) {
        let payload = payload.context("failed to parse WASM payload")?;
        match payload {
            Payload::ImportSection(reader) => {
                for group in reader {
                    let group = group.context("failed to parse WASM import")?;
                    let (functions, globals, memory) = count_imports(group)?;
                    imported_function_count += functions;
                    imported_global_count += globals;
                    if initial_memory_bytes.is_none() {
                        initial_memory_bytes = memory.map(memory_min_bytes).transpose()?;
                    }
                }
            }
            Payload::MemorySection(reader) => {
                for memory in reader {
                    let memory = memory.context("failed to parse WASM memory")?;
                    if initial_memory_bytes.is_none() {
                        initial_memory_bytes = Some(memory_min_bytes(memory)?);
                    }
                }
            }
            Payload::GlobalSection(reader) => {
                for global in reader {
                    let global = global.context("failed to parse WASM global")?;
                    let value = eval_i32_const_expr(&global.ty, &global.init_expr)?;
                    globals.push((value, global.ty.mutable));
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
            Payload::StartSection { func, .. } => start_function = Some(func),
            Payload::CodeSectionEntry(body) => {
                if start_function == Some(imported_function_count + local_function_index) {
                    start_body = Some(body);
                }
                local_function_index += 1;
            }
            Payload::DataSection(reader) => {
                for data in reader {
                    let data = data.context("failed to parse WASM data segment")?;
                    match &data.kind {
                        wasmparser::DataKind::Active {
                            memory_index: 0,
                            offset_expr,
                        } => {
                            let base = eval_i32_init_expr(offset_expr)? as u32;
                            writes.push(MemoryWrite::data(base, data.data)?);
                            data_segments.push(None);
                        }
                        wasmparser::DataKind::Passive => {
                            has_passive_data = true;
                            data_segments.push(Some(data.data));
                        }
                        _ => data_segments.push(None),
                    }
                }
            }
            _ => {}
        }
    }

    if meta_exports.is_empty() {
        bail!("no UNIFFI_META_* exports found in WASM file - is this a UniFFI crate?");
    }
    let memory_min = initial_memory_bytes.context("WASM module has no memory 0")?;
    for write in &writes {
        ensure_write_in_memory(write, memory_min)?;
    }

    let mut exports = Vec::with_capacity(meta_exports.len());
    let meta_global_indices: Vec<u32> = meta_exports.values().copied().collect();
    for (name, global_index) in meta_exports {
        let local_index = global_index
            .checked_sub(imported_global_count)
            .ok_or_else(|| {
                anyhow!(
                    "global index {global_index} refers to an imported global \
                 (not a local data pointer) for export '{name}'"
                )
            })?;
        let (addr, mutable) = *globals.get(local_index as usize).ok_or_else(|| {
            anyhow!("global index {global_index} out of range for export '{name}'")
        })?;
        ensure!(!mutable, "metadata export '{name}' is a mutable global");
        exports.push((name, addr as u32));
    }

    // Passive initializers run after active segments and can overwrite them.
    // Interpret the start whenever passive data is present so mixed modules
    // cannot silently return stale active bytes.
    if has_passive_data {
        ensure!(
            start_function.is_none() || start_body.is_some(),
            "WASM start function is imported; cannot infer passive metadata writes"
        );
        if let Some(body) = start_body {
            for write in read_start_writes(&body, &writes, &meta_global_indices, memory_min)? {
                match write {
                    StartupWrite::Init {
                        data_index,
                        destination,
                        source,
                        length,
                    } => {
                        let data = data_segments
                            .get(data_index as usize)
                            .and_then(|data| *data)
                            .with_context(|| {
                                format!(
                                    "memory.init refers to non-passive data segment {data_index}"
                                )
                            })?;
                        let source = source as usize;
                        let end = source
                            .checked_add(length as usize)
                            .context("memory.init source range overflow")?;
                        let bytes = data.get(source..end).with_context(|| {
                            format!("memory.init reads beyond data segment {data_index}")
                        })?;
                        let write = MemoryWrite::data(destination, bytes)?;
                        ensure_write_in_memory(&write, memory_min)?;
                        writes.push(write);
                    }
                    StartupWrite::Overwrite {
                        destination,
                        length,
                    } => {
                        let write = MemoryWrite::new(destination, length, None)?;
                        ensure_write_in_memory(&write, memory_min)?;
                        writes.push(write);
                    }
                }
            }
        }
    }

    let mut out = Vec::with_capacity(exports.len());
    for (name, addr) in exports {
        let bytes = read_from_data_segments(&writes, addr).with_context(|| {
            format!("failed to read metadata bytes for '{name}' at address {addr:#x}")
        })?;
        out.push((name, bytes));
    }

    Ok(out)
}

/// Count function/global imports and retain the first imported memory type.
fn count_imports(group: Imports<'_>) -> Result<(u32, u32, Option<MemoryType>)> {
    Ok(match group {
        Imports::Single(_, import) => (
            u32::from(matches!(
                import.ty,
                TypeRef::Func(_) | TypeRef::FuncExact(_)
            )),
            u32::from(matches!(import.ty, TypeRef::Global(_))),
            match import.ty {
                TypeRef::Memory(memory) => Some(memory),
                _ => None,
            },
        ),
        Imports::Compact1 { items, .. } => {
            let mut counts = (0u32, 0u32, None);
            for item in items {
                let item = item.context("failed to parse compact import item")?;
                counts.0 += u32::from(matches!(item.ty, TypeRef::Func(_) | TypeRef::FuncExact(_)));
                counts.1 += u32::from(matches!(item.ty, TypeRef::Global(_)));
                if counts.2.is_none() {
                    if let TypeRef::Memory(memory) = item.ty {
                        counts.2 = Some(memory);
                    }
                }
            }
            counts
        }
        Imports::Compact2 { ty, names, .. } => {
            let mut n = 0u32;
            for name in names {
                let _ = name.context("failed to parse compact import name")?;
                n += 1;
            }
            match ty {
                TypeRef::Func(_) | TypeRef::FuncExact(_) => (n, 0, None),
                TypeRef::Global(_) => (0, n, None),
                TypeRef::Memory(memory) => (0, 0, Some(memory)),
                _ => (0, 0, None),
            }
        }
    })
}

fn memory_min_bytes(memory: MemoryType) -> Result<u64> {
    ensure!(
        !memory.memory64,
        "memory64 is unsupported for UniFFI metadata pointers"
    );
    let page_size = 1u64
        .checked_shl(memory.page_size_log2())
        .context("WASM memory page size overflow")?;
    memory
        .initial
        .checked_mul(page_size)
        .context("WASM initial memory size overflow")
}

fn ensure_write_in_memory(write: &MemoryWrite<'_>, memory_min: u64) -> Result<()> {
    ensure!(
        u64::from(write.end()) <= memory_min,
        "WASM initializer writes beyond initial memory"
    );
    Ok(())
}

/// The wasm32 linker uses either a straight-line start or a three-block
/// once-only gate for shared memory. In the latter, a fresh zeroed guard makes
/// `cmpxchg` return 0 and `br_table 0 1 2` enter the initializer arm. We only
/// evaluate constant memory writes on that arm; other control flow is not safe
/// to infer from the binary without running it.
fn read_start_writes(
    body: &FunctionBody<'_>,
    active: &[MemoryWrite<'_>],
    meta_global_indices: &[u32],
    memory_min: u64,
) -> Result<Vec<StartupWrite>> {
    ensure!(
        body.as_bytes().len() <= MAX_START_BODY_BYTES,
        "WASM start function is too large for static metadata extraction"
    );
    ensure!(
        body.get_locals_reader()?.get_count() == 0,
        "WASM start function has local variables; cannot infer passive metadata"
    );
    let ops = body
        .get_operators_reader()?
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()?;

    let gated = matches!(ops.first(), Some(Operator::Block { .. }));
    let mut index = 0;
    if gated {
        for _ in 0..3 {
            ensure!(
                matches!(
                    ops.get(index),
                    Some(Operator::Block {
                        blockty: BlockType::Empty
                    })
                ),
                "unsupported shared-memory start gate"
            );
            index += 1;
        }
        let guard = constant(ops.get(index))?;
        index += 1;
        ensure!(
            constant(ops.get(index))? == 0,
            "unsupported start gate expected value"
        );
        index += 1;
        ensure!(
            constant(ops.get(index))? == 1,
            "unsupported start gate new value"
        );
        index += 1;
        ensure!(
            matches!(ops.get(index), Some(Operator::I32AtomicRmwCmpxchg { memarg }) if memarg.memory == 0 && memarg.offset == 0),
            "unsupported shared-memory start gate operation"
        );
        index += 1;
        let Some(Operator::BrTable { targets }) = ops.get(index) else {
            bail!("unsupported shared-memory start gate branch");
        };
        ensure!(
            targets
                .targets()
                .collect::<std::result::Result<Vec<_>, _>>()?
                == [0, 1]
                && targets.default() == 2,
            "unsupported shared-memory start gate branches"
        );
        index += 1;
        ensure!(
            matches!(ops.get(index), Some(Operator::End)),
            "start gate is not closed"
        );
        index += 1;

        let guard_end = guard
            .checked_add(4)
            .context("start guard address overflow")?;
        ensure!(
            u64::from(guard_end) <= memory_min,
            "WASM start guard is beyond initial memory"
        );
        ensure!(
            !active
                .iter()
                .any(|write| write.base < guard_end && guard < write.end()),
            "start guard is initialized by an active data segment"
        );
    }

    let mut stack = Vec::new();
    let mut writes = Vec::new();
    let mut dropped = Vec::new();
    loop {
        match ops.get(index) {
            Some(Operator::I32Const { value }) => stack.push(*value as u32),
            Some(Operator::GlobalSet { global_index }) => {
                ensure!(
                    !meta_global_indices.contains(global_index),
                    "WASM start mutates a metadata export global"
                );
                pop(&mut stack)?;
            }
            Some(Operator::MemoryInit { data_index, mem: 0 }) => {
                ensure!(
                    !dropped.contains(data_index),
                    "memory.init uses a previously dropped data segment"
                );
                let length = pop(&mut stack)?;
                let source = pop(&mut stack)?;
                let destination = pop(&mut stack)?;
                writes.push(StartupWrite::Init {
                    data_index: *data_index,
                    destination,
                    source,
                    length,
                });
            }
            Some(Operator::DataDrop { data_index }) => dropped.push(*data_index),
            Some(Operator::MemoryFill { mem: 0 }) => {
                let length = pop(&mut stack)?;
                pop(&mut stack)?; // fill value
                let destination = pop(&mut stack)?;
                writes.push(StartupWrite::Overwrite {
                    destination,
                    length,
                });
            }
            Some(Operator::I32AtomicStore { memarg })
                if memarg.memory == 0 && memarg.offset <= u32::MAX as u64 =>
            {
                pop(&mut stack)?; // stored value
                let destination = pop(&mut stack)?
                    .checked_add(memarg.offset as u32)
                    .context("atomic store address overflow")?;
                writes.push(StartupWrite::Overwrite {
                    destination,
                    length: 4,
                });
            }
            Some(Operator::MemoryAtomicNotify { memarg })
                if memarg.memory == 0 && memarg.offset == 0 =>
            {
                pop(&mut stack)?; // waiter count
                pop(&mut stack)?; // address
                index += 1;
                ensure!(
                    matches!(ops.get(index), Some(Operator::Drop)),
                    "atomic notification result is not discarded"
                );
            }
            Some(Operator::Br { relative_depth: 1 }) if gated => break,
            Some(Operator::End) if !gated => break,
            Some(other) => {
                bail!("unsupported WASM start operation for passive metadata: {other:?}")
            }
            None => bail!("unterminated WASM start function"),
        }
        index += 1;
    }
    ensure!(
        stack.is_empty(),
        "WASM start initializer leaves values on the stack"
    );
    index += 1;

    if gated {
        // The other branch waits for initialization by another thread. The
        // branch above jumps past it, then only data.drop operations remain.
        ensure!(
            matches!(ops.get(index), Some(Operator::End)),
            "start gate arm is not closed"
        );
        index += 1;
        ensure!(
            matches!(ops.get(index), Some(Operator::I32Const { .. })),
            "unsupported start gate wait address"
        );
        index += 1;
        ensure!(
            matches!(ops.get(index), Some(Operator::I32Const { value: 1 })),
            "unsupported start gate wait state"
        );
        index += 1;
        ensure!(
            matches!(ops.get(index), Some(Operator::I64Const { value: -1 })),
            "unsupported start gate wait timeout"
        );
        index += 1;
        ensure!(
            matches!(ops.get(index), Some(Operator::MemoryAtomicWait32 { memarg }) if memarg.memory == 0 && memarg.offset == 0),
            "unsupported start gate wait operation"
        );
        index += 1;
        ensure!(
            matches!(ops.get(index), Some(Operator::Drop)),
            "start gate wait result is not discarded"
        );
        index += 1;
        ensure!(
            matches!(ops.get(index), Some(Operator::End)),
            "start gate is not closed"
        );
        index += 1;
        while matches!(ops.get(index), Some(Operator::DataDrop { .. })) {
            index += 1;
        }
        ensure!(
            matches!(ops.get(index), Some(Operator::End)),
            "WASM start function is not closed"
        );
        index += 1;
    }
    ensure!(
        index == ops.len(),
        "unsupported operations after WASM start initializer"
    );
    Ok(writes)
}

fn constant(op: Option<&Operator<'_>>) -> Result<u32> {
    match op {
        Some(Operator::I32Const { value }) => Ok(*value as u32),
        _ => bail!("dynamic WASM start initializer operand"),
    }
}

fn pop(stack: &mut Vec<u32>) -> Result<u32> {
    stack
        .pop()
        .context("dynamic WASM start initializer operand")
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

fn read_from_data_segments<'a>(writes: &[MemoryWrite<'a>], addr: u32) -> Result<&'a [u8]> {
    let (index, write) = writes
        .iter()
        .enumerate()
        .rfind(|(_, write)| addr >= write.base && addr < write.end())
        .with_context(|| format!("address {addr:#x} was not initialized by a data segment"))?;
    let bytes = write
        .bytes
        .context("metadata address was overwritten by non-data memory write")?;
    // A later write may touch unrelated bytes near the end of this segment.
    // Return only the still-valid prefix; read_metadata detects a truncated
    // metadata item if its encoding actually crosses that boundary.
    let end = writes[index + 1..]
        .iter()
        .filter(|later| later.len != 0 && later.base > addr && later.base < write.end())
        .map(|later| later.base)
        .min()
        .unwrap_or_else(|| write.end());
    Ok(&bytes[(addr - write.base) as usize..(end - write.base) as usize])
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
    fn build_minimal_wasm(mutable_global: bool) -> Vec<u8> {
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
            0x7f,
            u8::from(mutable_global), // valtype i32, mutability
            0x41,
            0x80,
            0x08,
            0x0b, // i32.const 1024 ; end
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
        push_leb(out, payload.len() as u32);
        out.extend_from_slice(payload);
    }

    fn push_leb(out: &mut Vec<u8>, mut value: u32) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            out.push(byte | if value == 0 { 0 } else { 0x80 });
            if value == 0 {
                break;
            }
        }
    }

    fn build_passive_wasm(
        bodies: &[&[u8]],
        start: Option<u8>,
        imported_function: bool,
        active_prefix: bool,
    ) -> Vec<u8> {
        let mut wasm = Vec::from(&b"\0asm\x01\0\0\0"[..]);
        push_section(&mut wasm, 1, &[1, 0x60, 0, 0]);
        if imported_function {
            push_section(&mut wasm, 2, &[1, 1, b'm', 1, b'f', 0, 0]);
        }
        let mut functions = vec![bodies.len() as u8];
        functions.extend(std::iter::repeat_n(0, bodies.len()));
        push_section(&mut wasm, 3, &functions);
        // Shared memory permits the atomic once-only start gate used by Rust.
        push_section(&mut wasm, 5, &[1, 3, 1, 1]);
        push_section(&mut wasm, 6, &[1, 0x7f, 0, 0x41, 0x80, 0x08, 0x0b]);
        let name = b"UNIFFI_META_NS_demo";
        let mut exports = vec![1, name.len() as u8];
        exports.extend_from_slice(name);
        exports.extend_from_slice(&[3, 0]);
        push_section(&mut wasm, 7, &exports);
        if let Some(start) = start {
            push_section(&mut wasm, 8, &[start]);
        }
        push_section(&mut wasm, 12, &[if active_prefix { 2 } else { 1 }]);
        let mut code = vec![bodies.len() as u8];
        for body in bodies {
            push_leb(&mut code, body.len() as u32);
            code.extend_from_slice(body);
        }
        push_section(&mut wasm, 10, &code);
        let mut data = vec![if active_prefix { 2 } else { 1 }];
        if active_prefix {
            data.extend_from_slice(&[0, 0x41, 0x80, 0x08, 0x0b, 8]);
            data.extend_from_slice(&[9; 8]);
        }
        data.extend_from_slice(&[1, 8, 1, 2, 3, 4, 5, 6, 7, 8]);
        push_section(&mut wasm, 11, &data);
        wasmparser::Validator::new()
            .validate_all(&wasm)
            .expect("test WASM must validate");
        wasm
    }

    fn static_init(data_index: u8, destination: &[u8], source: u8, length: u8) -> Vec<u8> {
        let mut body = vec![0, 0x41]; // no locals; i32.const destination
        body.extend_from_slice(destination);
        body.extend_from_slice(&[0x41, source, 0x41, length, 0xfc, 8, data_index, 0, 0x0b]);
        body
    }

    #[test]
    fn extracts_meta_blob_from_minimal_wasm() {
        let wasm = build_minimal_wasm(false);
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
    fn rejects_mutable_metadata_global() {
        let wasm = build_minimal_wasm(true);
        let err = read_meta_blobs(&wasm).expect_err("exported metadata pointer must be immutable");
        assert!(err.to_string().contains("mutable global"), "{err:#}");
    }

    #[test]
    fn extracts_passive_data_initialized_by_start() {
        let body = static_init(0, &[0x80, 0x08], 0, 8);
        let wasm = build_passive_wasm(&[&body], Some(0), false, false);
        let blobs = read_meta_blobs(&wasm).expect("start initializes passive metadata");
        assert_eq!(&blobs[0].1[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn extracts_passive_data_before_straight_line_drop() {
        let mut body = static_init(0, &[0x80, 0x08], 0, 8);
        body.pop(); // replace function end with data.drop 0; end
        body.extend_from_slice(&[0xfc, 9, 0, 0x0b]);
        let wasm = build_passive_wasm(&[&body], Some(0), false, false);
        let blobs = read_meta_blobs(&wasm).expect("data.drop follows memory.init");
        assert_eq!(&blobs[0].1[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn ignores_memory_init_in_uncalled_function() {
        let empty_start = [0, 0x0b];
        let uncalled = static_init(0, &[0x80, 0x08], 0, 8);
        let wasm = build_passive_wasm(&[&empty_start, &uncalled], Some(1), true, false);
        let err = read_meta_blobs(&wasm).expect_err("uncalled function cannot initialize metadata");
        assert!(format!("{err:#}").contains("not initialized"), "{err:#}");
    }

    #[test]
    fn rejects_dynamic_and_conditional_start_initializers() {
        let dynamic = [0, 0x23, 0, 0x41, 0, 0x41, 8, 0xfc, 8, 0, 0, 0x0b];
        let conditional = [
            0, 0x41, 0, 0x04, 0x40, 0x41, 0x80, 0x08, 0x41, 0, 0x41, 8, 0xfc, 8, 0, 0, 0x0b, 0x0b,
        ];
        for body in [&dynamic[..], &conditional[..]] {
            let wasm = build_passive_wasm(&[body], Some(0), false, false);
            let err = read_meta_blobs(&wasm).expect_err("ambiguous start must fail closed");
            assert!(
                err.to_string().contains("unsupported WASM start operation"),
                "{err:#}"
            );
        }
    }

    #[test]
    fn rejects_out_of_bounds_memory_init_source() {
        let body = static_init(0, &[0x80, 0x08], 2, 8);
        let wasm = build_passive_wasm(&[&body], Some(0), false, false);
        let err = read_meta_blobs(&wasm).expect_err("out-of-bounds source must fail");
        assert!(
            err.to_string().contains("reads beyond data segment"),
            "{err:#}"
        );
    }

    #[test]
    fn rejects_memory_init_beyond_initial_memory() {
        let body = static_init(0, &[0xfa, 0xff, 0x03], 0, 8); // 65530 + 8 > one page
        let wasm = build_passive_wasm(&[&body], Some(0), false, false);
        let err = read_meta_blobs(&wasm).expect_err("write would trap at instantiation");
        assert!(err.to_string().contains("beyond initial memory"), "{err:#}");
    }

    #[test]
    fn later_write_truncates_data_at_its_first_modified_byte() {
        let first = [9; 16];
        let second = [1, 2, 3, 4];
        let writes = [
            MemoryWrite::data(1024, &first).unwrap(),
            MemoryWrite::data(1036, &second).unwrap(),
        ];
        assert_eq!(read_from_data_segments(&writes, 1024).unwrap(), &[9; 12]);
    }

    #[test]
    fn zero_length_write_does_not_truncate_metadata() {
        let first = [9; 8];
        let writes = [
            MemoryWrite::data(1024, &first).unwrap(),
            MemoryWrite::data(1028, &[]).unwrap(),
        ];
        assert_eq!(read_from_data_segments(&writes, 1024).unwrap(), &first);
    }

    #[test]
    fn mixed_active_passive_with_unmodeled_start_fails_closed() {
        let conditional_start = [0, 0x41, 0, 0x04, 0x40, 0x0b, 0x0b];
        let wasm = build_passive_wasm(&[&conditional_start], Some(0), false, true);
        let err = read_meta_blobs(&wasm).expect_err("unknown start might overwrite active data");
        assert!(err.to_string().contains("unsupported WASM start operation"));
    }

    #[test]
    fn imported_start_cannot_hide_passive_overwrite() {
        let wasm = build_passive_wasm(&[], Some(0), true, true);
        let err = read_meta_blobs(&wasm).expect_err("imported start may alter active metadata");
        assert!(
            err.to_string().contains("start function is imported"),
            "{err:#}"
        );
    }

    #[test]
    fn passive_start_overwrites_active_metadata() {
        let body = static_init(1, &[0x80, 0x08], 0, 8);
        let wasm = build_passive_wasm(&[&body], Some(0), false, true);
        let blobs = read_meta_blobs(&wasm).expect("start overwrite is the final metadata value");
        assert_eq!(&blobs[0].1[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn extracts_passive_data_through_shared_memory_start_gate() {
        // Three nested blocks and cmpxchg select the initializer on fresh
        // memory. The alternate arm only waits for another thread.
        let body = [
            0, 0x02, 0x40, 0x02, 0x40, 0x02, 0x40, 0x41, 0x80, 0x20, 0x41, 0, 0x41, 1, 0xfe, 0x48,
            2, 0, 0x0e, 2, 0, 1, 2, 0x0b, 0x41, 0x80, 0x08, 0x41, 0, 0x41, 8, 0xfc, 8, 0, 0, 0x41,
            0x80, 0x20, 0x41, 2, 0xfe, 0x17, 2, 0, 0x41, 0x80, 0x20, 0x41, 0x7f, 0xfe, 0, 2, 0,
            0x1a, 0x0c, 1, 0x0b, 0x41, 0x80, 0x20, 0x41, 1, 0x42, 0x7f, 0xfe, 1, 2, 0, 0x1a, 0x0b,
            0xfc, 9, 0, 0x0b,
        ];
        let wasm = build_passive_wasm(&[&body], Some(0), false, false);
        let blobs = read_meta_blobs(&wasm).expect("shared-memory start initializes passive data");
        assert_eq!(&blobs[0].1[..8], &[1, 2, 3, 4, 5, 6, 7, 8]);
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
