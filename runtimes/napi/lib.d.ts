/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// @ubjs/core publishes ESM types only, so this CommonJS file asks for them as an import.
import type {
  FfiType as CoreFfiType,
  ModuleDefinitions,
} from "@ubjs/core" with { "resolution-mode": "import" };
export {
  resolveLibPath,
  ResolveLibPathError,
  type ResolveLibPathOptions,
} from "./typescript/dist/resolve-lib.js";

/** What @ubjs/wasm's registerSync returns; structurally @ubjs/worker's RegisteredPlayer. */
export type NativeModuleInterface = Record<string, (...args: any[]) => any> & {
  rustbuffer_alloc(n: number): Uint8Array;
  rustbuffer_free(view: Uint8Array): void;
};

/** A table for register(). A library with no callbacks or no structs may leave them out. */
export type RegisterDefinitions = Omit<
  ModuleDefinitions,
  "callbacks" | "structs"
> &
  Partial<Pick<ModuleDefinitions, "callbacks" | "structs">>;

/**
 * Loads a UniFFI shared library: `open(path)`, then `register(definitions)`
 * for a player whose methods call the library's functions.
 */
export declare class UniffiNativeModule {
  /**
   * Checks the library path with a trial `dlopen`, so an invalid path fails
   * here rather than at `register()`. The library stays mapped.
   */
  static open(path: string): UniffiNativeModule;
  /**
   * Opens the library, resolves every symbol in `definitions`, and returns a
   * player bound to them. A function def carrying `async` becomes one entry,
   * `(...args, status, signal) => Promise`, that runs UniFFI's poll loop.
   */
  register(definitions: RegisterDefinitions): NativeModuleInterface;
  /**
   * Drains in-flight calls and aborts callbacks. `{ force: true }` also
   * `dlclose`s the library. An async call still in flight stays pending.
   */
  unload(options?: { force?: boolean } | null): void;
}
export declare const FfiType: typeof CoreFfiType;
