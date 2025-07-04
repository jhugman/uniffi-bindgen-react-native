// Generated by uniffi-bindgen-react-native
// Export the generated bindings to the app.
{%- let root = self.project_root() %}
{%- let bindings = self.config.project.wasm_bindings_ts_path(root) %}
{%- let bindings = self.relative_to(root, bindings) %}
{%- for m in self.config.modules %}
export * from './{{ bindings }}/{{ m.ts() }}';
{%- endfor %}

// Now import the bindings so we can:
// - intialize them
// - export them as namespaced objects as the default export.
{%- for m in self.config.modules %}
import * as {{ m.ts() }} from './{{ bindings }}/{{ m.ts() }}';
{%- endfor %}

import initAsync from './{{ bindings }}/wasm-bindgen/index.js';
import wasmPath from './{{ bindings }}/wasm-bindgen/index_bg.wasm';

export async function uniffiInitAsync() {
  await initAsync({ module_or_path: wasmPath })

  // Initialize the generated bindings: mostly checksums, but also callbacks.
  // - the boolean flag ensures this loads exactly once, even if the JS code
  //   is reloaded (e.g. during development with metro).
  {%- for m in self.config.modules %}
  {{ m.ts() }}.default.initialize();
  {%- endfor %}
}

// Export the crates as individually namespaced objects.
export default {
{%- for m in self.config.modules %}
  {{ m.ts() }},
{%- endfor %}
};

{# space #}
