// Generated by uniffi-bindgen-react-native
import installer from './{{ self.config.project.codegen_filename() }}';

// Register the rust crate with Hermes
installer.installRustCrate();

// Export the generated bindings to the app.
{%- let root = self.project_root() %}
{%- let bindings = self.config.project.bindings.ts_path(root) %}
{%- let bindings = self.relative_to(root, bindings) %}
{%- for m in self.config.modules %}
export * from './{{ bindings }}/{{ m.ts() }}';
{%- endfor %}

// Initialize the generated bindings: mostly checksums, but also callbacks.
{%- for m in self.config.modules %}
import {{ m.ts() }}_ from './{{ bindings }}/{{ m.ts() }}';
{%- endfor %}
{% for m in self.config.modules %}
{{ m.ts() }}_.initialize();
{%- endfor %}
{# space #}