{{- self.import_infra_type("UniffiObjectInterface", "objects") -}}
{{- self.import_infra_type("UnsafeMutableRawPointer", "objects") -}}
{{- self.import_infra("FfiConverterObject", "objects") -}}
{{- self.import_infra_type("UniffiObjectFactory", "objects") -}}
{{- self.import_infra_type("FfiConverter", "ffi-converters") -}}
{{- self.import_infra_type("UniffiRustArcPtr", "rust-call") }}
{{- self.import_infra_type("UniffiRustCallStatus", "rust-call") }}
