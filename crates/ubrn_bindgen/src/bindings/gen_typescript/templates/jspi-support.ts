// Evaluate before wasm-bindgen glue, which may construct suspending imports.
const wasm = (globalThis as any).WebAssembly;
if (typeof wasm?.promising !== "function" || typeof wasm?.Suspending !== "function") {
  throw new Error(
    "These bindings require WebAssembly JSPI. Use a JSPI-capable runtime; on Node 24 enable --experimental-wasm-jspi and --experimental-wasm-exnref.",
  );
}
export {};
