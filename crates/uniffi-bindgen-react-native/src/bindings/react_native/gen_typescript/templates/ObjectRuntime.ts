{{- self.import_infra("AbstractUniffiObject", "objects") -}}
{{- self.import_infra_type("UnsafeMutableRawPointer", "objects") -}}
{{- self.import_infra("FfiConverterObject", "objects") -}}
{{- self.import_infra_type("UniffiObjectFactory", "objects") -}}
{{- self.import_infra_type("FfiConverter", "ffi-converters") -}}
{{- self.import_infra_type("UniffiRustArcPtr", "rust-call") }}
{{- self.import_infra_type("UniffiRustCallStatus", "rust-call") }}
{{- self.import_infra_type("UniffiRustArcPtrDestructor", "rust-call") }}

const uniffiBlessPointer = (p: UnsafeMutableRawPointer, d: UniffiRustArcPtrDestructor): UniffiRustArcPtr => {
    return rustCall((status) => NativeModule.{{ ci.ffi_function_bless_pointer().name() }}(p, d, status));
};
