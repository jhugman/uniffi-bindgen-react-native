/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
//! UniFFI's fixed future ABI: the shapes of poll, cancel, complete and free
//! do not depend on the engine, so every player adds them the same way.
use crate::spec::{CallbackDef, FunctionDef, ModuleSpec};
use crate::{Error, FfiTypeDesc, Result};

pub const RUST_FUTURE_CONTINUATION: &str = "RustFutureContinuationCallback";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncFunctionDef {
    pub poll: String,
    pub cancel: String,
    pub complete: String,
    pub free: String,
    pub ret: FfiTypeDesc,
}

impl AsyncFunctionDef {
    pub fn exports(&self) -> [&str; 4] {
        [&self.poll, &self.cancel, &self.complete, &self.free]
    }
}

fn handle_only(ret: FfiTypeDesc, has_rust_call_status: bool) -> FunctionDef {
    FunctionDef {
        args: vec![FfiTypeDesc::Handle],
        ret,
        has_rust_call_status,
    }
}

fn add_function(spec: &mut ModuleSpec, name: &str, def: FunctionDef) -> Result<()> {
    match spec.functions.get(name) {
        Some(existing) if *existing != def => Err(Error::Other(format!(
            "async export `{name}` is declared as {existing:?} and also needed as {def:?}"
        ))),
        Some(_) => Ok(()),
        None => {
            spec.functions.insert(name.to_string(), def);
            Ok(())
        }
    }
}

/// Adds each entry's four exports, and the continuation callback they share,
/// to `spec`. Call before `Module::new` so a missing export fails there.
pub fn add_async_entries<'a>(
    spec: &mut ModuleSpec,
    entries: impl IntoIterator<Item = &'a AsyncFunctionDef>,
) -> Result<()> {
    let mut any = false;
    for e in entries {
        any = true;
        add_function(
            spec,
            &e.poll,
            FunctionDef {
                args: vec![
                    FfiTypeDesc::Handle,
                    FfiTypeDesc::Callback(RUST_FUTURE_CONTINUATION.to_string()),
                    FfiTypeDesc::Handle,
                ],
                ret: FfiTypeDesc::Void,
                has_rust_call_status: false,
            },
        )?;
        add_function(spec, &e.cancel, handle_only(FfiTypeDesc::Void, false))?;
        add_function(spec, &e.free, handle_only(FfiTypeDesc::Void, false))?;
        add_function(spec, &e.complete, handle_only(e.ret.clone(), true))?;
    }
    if any {
        spec.callbacks.insert(
            RUST_FUTURE_CONTINUATION.to_string(),
            CallbackDef {
                args: vec![FfiTypeDesc::UInt64, FfiTypeDesc::Int8],
                ret: FfiTypeDesc::Void,
                has_rust_call_status: false,
                out_return: false,
            },
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::RustBufferSymbols;
    use FfiTypeDesc::*;

    fn spec() -> ModuleSpec {
        ModuleSpec {
            rustbuffer_symbols: RustBufferSymbols {
                alloc: "a".into(),
                free: "f".into(),
                from_bytes: "b".into(),
            },
            functions: Default::default(),
            callbacks: Default::default(),
            structs: Default::default(),
        }
    }

    fn entry(complete: &str, ret: FfiTypeDesc) -> AsyncFunctionDef {
        AsyncFunctionDef {
            poll: "p".into(),
            cancel: "c".into(),
            complete: complete.into(),
            free: "f".into(),
            ret,
        }
    }

    fn fd(args: Vec<FfiTypeDesc>, ret: FfiTypeDesc, status: bool) -> FunctionDef {
        FunctionDef {
            args,
            ret,
            has_rust_call_status: status,
        }
    }

    #[test]
    fn adds_the_fixed_shapes_for_each_export() {
        let mut s = spec();
        add_async_entries(&mut s, [&entry("k", UInt32)]).unwrap();
        assert_eq!(
            s.functions["p"],
            fd(
                vec![Handle, Callback(RUST_FUTURE_CONTINUATION.into()), Handle],
                Void,
                false
            )
        );
        assert_eq!(s.functions["c"], fd(vec![Handle], Void, false));
        assert_eq!(s.functions["f"], fd(vec![Handle], Void, false));
        assert_eq!(s.functions["k"], fd(vec![Handle], UInt32, true));
        assert_eq!(
            s.callbacks[RUST_FUTURE_CONTINUATION],
            CallbackDef {
                args: vec![UInt64, Int8],
                ret: Void,
                has_rust_call_status: false,
                out_return: false
            }
        );
    }

    #[test]
    fn the_players_continuation_replaces_the_tables() {
        let mut s = spec();
        s.callbacks.insert(
            RUST_FUTURE_CONTINUATION.into(),
            CallbackDef {
                args: vec![UInt32],
                ret: Void,
                has_rust_call_status: false,
                out_return: false,
            },
        );
        add_async_entries(&mut s, [&entry("k", UInt32)]).unwrap();
        assert_eq!(
            s.callbacks[RUST_FUTURE_CONTINUATION].args,
            vec![UInt64, Int8]
        );
    }

    #[test]
    fn entries_sharing_exports_add_each_once() {
        let mut s = spec();
        add_async_entries(&mut s, [&entry("k1", UInt32), &entry("k2", RustBuffer)]).unwrap();
        assert_eq!(s.functions.len(), 5);
    }

    #[test]
    fn two_entries_naming_one_complete_must_agree_on_ret() {
        let mut s = spec();
        let err =
            add_async_entries(&mut s, [&entry("k", UInt32), &entry("k", RustBuffer)]).unwrap_err();
        assert!(err.to_string().contains("`k`"));
    }

    #[test]
    fn an_export_the_table_already_has_must_have_the_same_shape() {
        let mut s = spec();
        s.functions.insert("k".into(), fd(vec![Handle], Int8, true));
        let err = add_async_entries(&mut s, [&entry("k", UInt32)]).unwrap_err();
        assert!(err.to_string().contains("`k`"));
        add_async_entries(&mut s, [&entry("k", Int8)]).unwrap();
    }

    #[test]
    fn no_entries_leave_the_spec_alone() {
        let mut s = spec();
        add_async_entries(&mut s, []).unwrap();
        assert!(s.callbacks.is_empty() && s.functions.is_empty());
    }
}
