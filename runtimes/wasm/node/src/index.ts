/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { readFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  UniffiNativeModule,
  type ImportResolver,
  type WasmSource,
  type ModuleDefinitions,
  type NativeModuleInterface,
} from "../../core/src/module.js";

export { FfiType } from "../../core/src/ffi-type.js";
export { UniffiNativeModule };
export type { WasmSource };

/**
 * `options.resolveModule` supplies the wasm's own module imports. A cdylib
 * that reached wasm-bindgen through its dependencies imports `./<lib>_bg.js`,
 * and the caller must hand that glue over — generated bindings import it
 * statically and pass it here. Without it, instantiation fails naming the
 * import it could not satisfy.
 */
export async function openWasm(
  source: WasmSource,
  options?: { resolveModule?: ImportResolver },
): Promise<UniffiNativeModule> {
  if (typeof source === "string") {
    const bytes = await readFile(source);
    return UniffiNativeModule.open(bytes, options);
  }
  if (source instanceof URL) {
    const bytes = await readFile(fileURLToPath(source));
    return UniffiNativeModule.open(bytes, options);
  }
  return UniffiNativeModule.open(source, options);
}

/**
 * Node-only sync registration over an already-instantiated source. The fixture
 * test runner uses this path via wasm-as-ESM imports.
 */
export function registerSync(
  mod: UniffiNativeModule,
  defs: ModuleDefinitions,
): NativeModuleInterface {
  return mod.registerSync(defs);
}
