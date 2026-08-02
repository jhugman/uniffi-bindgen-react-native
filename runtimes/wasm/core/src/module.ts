/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { Memory } from "./memory.js";
import { Scratch } from "./scratch.js";
import {
  compileStructLayout,
  type FieldDesc,
  type StructLayout,
} from "./marshal.js";
import {
  specializeFunction,
  getCapacityHint,
  canUseFunctionConstructor,
  type DispatchContext,
  type FunctionDef,
} from "./call.js";
import { CallbackTable, type CallbackDef } from "./callback.js";
import { FutureRegistry } from "./future.js";

export type WasmSource =
  | WebAssembly.Module
  | ArrayBuffer
  | Uint8Array
  | Response
  | URL
  | string;

export interface ModuleDefinitions {
  symbols: {
    rustbuffer_alloc: string;
    rustbuffer_free: string;
    rustbuffer_from_bytes: string;
  };
  functions: Record<string, FunctionDef>;
  callbacks: Record<string, CallbackDef>;
  structs: Record<string, FieldDesc[]>;
}

export type NativeModuleInterface = Record<string, (...args: any[]) => any>;

const REQUIRED_EXPORTS = ["__ubrn_alloc", "__ubrn_free", "memory"] as const;

const SCRATCH_BYTES = 64 * 1024;

/**
 * Resolver for unrecognized wasm import modules. The runner side may run
 * `wasm-bindgen-cli --target bundler` against the cdylib, leaving the
 * rewritten wasm with imports against `./<crate>_bg.js`. The env-specific
 * entry (node/browser) supplies a resolver that dynamic-imports those
 * relative module paths and hands the named exports back. Any module the
 * resolver can't satisfy is filled with throwing stubs so calling into a
 * truly-unwired path fails loudly rather than silently no-op'ing.
 */
export type ImportResolver = (
  moduleName: string,
) => Promise<Record<string, unknown> | undefined>;

async function buildImports(
  compiled: WebAssembly.Module,
  importObject: WebAssembly.Imports,
  resolveModule: ImportResolver | undefined,
): Promise<{ resolved: Map<string, Record<string, unknown>> }> {
  const resolved = new Map<string, Record<string, unknown>>();
  const seenModules = new Set<string>();
  for (const imp of WebAssembly.Module.imports(compiled)) {
    seenModules.add(imp.module);
  }
  if (resolveModule) {
    for (const moduleName of seenModules) {
      if (moduleName in importObject) continue;
      const exports = await resolveModule(moduleName);
      if (exports) {
        resolved.set(moduleName, exports);
        (importObject as Record<string, unknown>)[moduleName] = exports;
      }
    }
  }
  for (const imp of WebAssembly.Module.imports(compiled)) {
    const slotKey = imp.module;
    let slot = (importObject as Record<string, Record<string, unknown>>)[
      slotKey
    ];
    if (slot && imp.name in slot) continue;
    if (!slot) {
      slot = {};
      (importObject as Record<string, Record<string, unknown>>)[slotKey] = slot;
    }
    switch (imp.kind) {
      case "function":
        slot[imp.name] = (..._args: any[]) => {
          throw new Error(
            `wasm import ${imp.module}.${imp.name} is a stub — ` +
              `no resolver provided exports for "${imp.module}".`,
          );
        };
        break;
      case "global":
        slot[imp.name] = new WebAssembly.Global(
          { value: "i32", mutable: false },
          0,
        );
        break;
      case "memory":
        slot[imp.name] = new WebAssembly.Memory({ initial: 1 });
        break;
      case "table":
        slot[imp.name] = new WebAssembly.Table({
          initial: 0,
          element: "anyfunc",
        });
        break;
    }
  }
  return { resolved };
}

/** Install a JS function in `__indirect_function_table` to receive Rust
 * panic messages, and tell the helper crate's panic-log slot to dispatch
 * to it. The helper crate doesn't import an env-namespaced
 * `__ubrn_panic_log` (that would force every linking cdylib to declare an
 * unresolvable env import — see `helper-crate/src/lib.rs`); the player
 * wires it up here via the same trampoline machinery callbacks use. */
function installPanicLog(
  callbacks: CallbackTable,
  memory: Memory,
  exports: WebAssembly.Exports,
): void {
  const setSlot = exports.__ubrn_set_panic_log as
    | ((idx: number) => void)
    | undefined;
  if (typeof setSlot !== "function") return;
  const panicLogDef: CallbackDef = {
    args: [{ tag: "UInt32" } as const, { tag: "UInt32" } as const],
    ret: { tag: "Void" } as const,
    hasRustCallStatus: false,
  };
  const slot = callbacks.installCallbackFunction((ptr: number, len: number) => {
    const bytes = memory.readBytes(ptr, len);
    const msg = new TextDecoder().decode(bytes);
    // eslint-disable-next-line no-console
    console.error("[Rust panic] " + msg + "\n" + new Error().stack);
  }, panicLogDef);
  setSlot(slot);
}

export class UniffiNativeModule {
  readonly memory: Memory;
  readonly scratch: Scratch;
  readonly exports: WebAssembly.Exports;
  readonly instance: WebAssembly.Instance;
  readonly callbacks: CallbackTable;
  readonly futures: FutureRegistry;
  readonly alloc: (size: number, align: number) => number;
  readonly free: (ptr: number, size: number, align: number) => void;

  private constructor(instance: WebAssembly.Instance) {
    this.instance = instance;
    this.exports = instance.exports;

    for (const name of REQUIRED_EXPORTS) {
      if (!(name in this.exports)) {
        throw new Error(
          `UniffiNativeModule.open: required export "${name}" not found in wasm module`,
        );
      }
    }

    const wm = this.exports.memory as WebAssembly.Memory;
    this.memory = new Memory(wm);

    const allocFn = this.exports.__ubrn_alloc as (
      size: number,
      align: number,
    ) => number;
    const freeFn = this.exports.__ubrn_free as (
      ptr: number,
      size: number,
      align: number,
    ) => void;
    this.alloc = allocFn;
    this.free = freeFn;

    const arenaPtr = allocFn(SCRATCH_BYTES, 8);
    if (arenaPtr === 0) {
      throw new Error(
        "UniffiNativeModule.open: __ubrn_alloc returned 0 for scratch arena",
      );
    }
    this.scratch = new Scratch(
      arenaPtr,
      SCRATCH_BYTES,
      (size) => allocFn(size, 8),
      (ptr, size) => freeFn(ptr, size, 8),
    );

    // Build the callback dispatch infrastructure first — `installPanicLog`
    // emits a trampoline through it, and the `__ubrn_dispatch` import
    // closure forwards to `callbacks.dispatch` so any later panic from
    // inside an FFI call routes correctly.
    this.callbacks = new CallbackTable(this.memory, this.exports, true);
    this.futures = new FutureRegistry(this.callbacks);

    installPanicLog(this.callbacks, this.memory, this.exports);
    if (typeof this.exports.__ubrn_install_panic_hook === "function") {
      (this.exports.__ubrn_install_panic_hook as () => void)();
    }
  }

  static async open(
    source: WasmSource,
    options?: { resolveModule?: ImportResolver },
  ): Promise<UniffiNativeModule> {
    let compiled: WebAssembly.Module;
    if (source instanceof WebAssembly.Module) {
      compiled = source;
    } else if (source instanceof ArrayBuffer || source instanceof Uint8Array) {
      compiled = await WebAssembly.compile(source as BufferSource);
    } else if (typeof Response !== "undefined" && source instanceof Response) {
      compiled = await WebAssembly.compileStreaming(source);
    } else {
      throw new Error(
        "UniffiNativeModule.open: URL/string sources must be resolved by the env-specific entry",
      );
    }

    // Per-instance imports: the closures below close over `created`, which we
    // populate after instantiation. They will reject if invoked during the
    // instantiation start function (start-section panics happen before we have
    // a UniffiNativeModule to dispatch through).
    let created: UniffiNativeModule | null = null;

    const importObject: WebAssembly.Imports = {
      env: {
        __ubrn_dispatch: (...args: any[]) => {
          const mod = created;
          if (!mod) {
            throw new Error(
              "__ubrn_dispatch invoked before module construction completed",
            );
          }
          return mod.callbacks.dispatch(...(args as [number, ...any[]]));
        },
      },
    };
    const { resolved } = await buildImports(
      compiled,
      importObject,
      options?.resolveModule,
    );

    const instance = await WebAssembly.instantiate(compiled, importObject);

    // wasm-bindgen-bundler glue files expose `__wbg_set_wasm` so JS-side
    // helpers can reach back into the wasm exports. Wire that up before
    // calling any exports, then run `__wbindgen_start` (wasm-bindgen's
    // initializer; it sets up the panic hook the wbg-glued bindings need).
    for (const exports of resolved.values()) {
      const setter = exports["__wbg_set_wasm"];
      if (typeof setter === "function") {
        (setter as (w: WebAssembly.Exports) => void)(instance.exports);
      }
    }
    const startFn = instance.exports["__wbindgen_start"];
    if (typeof startFn === "function") {
      (startFn as () => void)();
    }

    created = new UniffiNativeModule(instance);
    return created;
  }

  async register(
    definitions: ModuleDefinitions,
    opts?: { disableJit?: boolean },
  ): Promise<NativeModuleInterface> {
    return this.registerSync(definitions, opts);
  }

  registerSync(
    definitions: ModuleDefinitions,
    opts?: { disableJit?: boolean },
  ): NativeModuleInterface {
    if (!definitions.symbols.rustbuffer_alloc) {
      throw new Error(
        "register: definitions.symbols.rustbuffer_alloc must be a non-empty string",
      );
    }
    if (!definitions.symbols.rustbuffer_free) {
      throw new Error(
        "register: definitions.symbols.rustbuffer_free must be a non-empty string",
      );
    }
    const structs = new Map<string, StructLayout>();
    for (const [name, fields] of Object.entries(definitions.structs)) {
      structs.set(name, compileStructLayout(fields));
    }
    const callbackDefs = new Map(Object.entries(definitions.callbacks));
    // Hand the resolution context to the CallbackTable so that when a
    // wasm-to-JS dispatch lifts a `Callback`-typed arg (a function-table
    // index Rust passed in), we can wrap it in a JS callable that lowers
    // its args via the registered shape's `args` and the compiled struct
    // layouts.
    this.callbacks.setRegistrationContext({ callbackDefs, structs });
    // Probe once at register time: under strict CSP `new Function` throws,
    // and the caller may explicitly opt out via `disableJit` (used by tests
    // that want to exercise the interpreted path).
    const useJit = !opts?.disableJit && canUseFunctionConstructor();
    const ctx: DispatchContext = {
      memory: this.memory,
      scratch: this.scratch,
      structs,
      callbackDefs,
      alloc: this.alloc,
      free: this.free,
      installCallback: (fn, def) =>
        this.callbacks.installCallbackFunction(fn, def),
      useJit,
    };

    const result: NativeModuleInterface = Object.create(null);
    for (const [fnName, def] of Object.entries(definitions.functions)) {
      const exportFn = this.exports[fnName];
      if (typeof exportFn !== "function") {
        throw new Error(`register: wasm export "${fnName}" not found`);
      }
      result[fnName] = specializeFunction(ctx, exportFn, def, fnName);
    }

    // JS-callable allocator pair, exposed alongside the per-function entries.
    // A converter handed `rustbuffer_alloc` as its `lower(value, alloc)`
    // writes its payload straight into a view over wasm memory;
    // `writeRustBufferPayload` then spots the wasm-aliased view and forwards
    // `(byteOffset, byteLength)` into the `RustBuffer` struct, so neither
    // side copies.
    //
    // The helper crate's `__ubrn_alloc` / `__ubrn_free` back this rather than
    // `ffi_<crate>_rustbuffer_alloc`. Both reach the same Rust global
    // allocator, so Rust can still take ownership via `Vec::from_raw_parts`,
    // and the path stays crate-agnostic — no per-crate symbol lookup at
    // register time.
    const memory = this.memory;
    const alloc = this.alloc;
    const free = this.free;
    /**
     * Allocate `n` bytes of wasm linear memory and return a Uint8Array view
     * over them. The view aliases wasm memory directly — no copy.
     *
     * Lifetime: the caller MUST call `rustbuffer_free(view)` before any other
     * wasm operation that could trigger `WebAssembly.Memory.grow` (which
     * detaches the underlying ArrayBuffer). Common detaching operations
     * include subsequent `rustbuffer_alloc` calls and FFI calls that allocate
     * Rust-side. After detachment, the view's byteLength becomes 0 and
     * `rustbuffer_free(view)` will leak the original allocation.
     */
    result.rustbuffer_alloc = (n: number): Uint8Array => {
      if (n === 0) return new Uint8Array(0);
      const ptr = alloc(n, 1);
      return new Uint8Array(memory.buffer(), ptr, n);
    };
    result.rustbuffer_free = (view: Uint8Array): void => {
      const hint = getCapacityHint(view);
      if (view.byteLength === 0 && hint === undefined) return;
      if ((view.buffer as ArrayBuffer).byteLength === 0) {
        throw new Error(
          "rustbuffer_free: view is detached (held across a wasm memory growth?)",
        );
      }
      // Rust may over-allocate a returned RustBuffer, so its capacity can
      // exceed the view's byteLength. The handoff path stashes the real
      // capacity via `setCapacityHint`. Views from `rustbuffer_alloc` carry no
      // hint because their length already is the allocation size.
      const cap = hint ?? view.byteLength;
      if (cap === 0) return;
      free(view.byteOffset, cap, 1);
    };

    return result;
  }
}
