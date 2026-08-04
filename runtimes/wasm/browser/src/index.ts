/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import {
  UniffiNativeModule,
  type ImportResolver,
  type WasmSource,
} from "../../core/src/module.js";
export { FfiType } from "../../core/src/ffi-type.js";
export { UniffiNativeModule };
export type { WasmSource };

/**
 * `options.resolveModule` supplies the wasm's own module imports. A cdylib
 * that reached wasm-bindgen through its dependencies imports `./<lib>_bg.js`,
 * and the caller must hand that glue over — generated bindings import it
 * statically, which keeps it in the bundler's graph. Nothing here loads a
 * module at runtime, so a bundler sees no dynamic import to warn about.
 */
export async function openWasm(
  source: WasmSource | Promise<Response>,
  options?: { resolveModule?: ImportResolver },
): Promise<UniffiNativeModule> {
  if (typeof source === "string" || source instanceof URL) {
    const response = await fetch(source.toString());
    return UniffiNativeModule.open(response, options);
  }
  // `fetch(...)` handed straight in, without an intervening `await`.
  if (typeof (source as Promise<Response>)?.then === "function") {
    return UniffiNativeModule.open(
      await (source as Promise<Response>),
      options,
    );
  }
  return UniffiNativeModule.open(source as WasmSource, options);
}
