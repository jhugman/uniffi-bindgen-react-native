(() => {
    // Get the bindings contract version from our ComponentInterface
    const bindingsContractVersion = {{ ci.uniffi_contract_version() }};
    // Get the scaffolding contract version by calling the into the dylib
    const scaffoldingContractVersion = nativeModule().{{ ci.ffi_uniffi_contract_version().name() }}();
    if (bindingsContractVersion != scaffoldingContractVersion) {
        throw new Error("contractVersionMismatch");
    }

    {%- for (name, expected_checksum) in ci.iter_checksums() %}
    if (nativeModule().{{ name }}() != {{ expected_checksum }}) {
        throw new Error("apiChecksumMismatch: {{ name }}");
    }
    {%- endfor %}

    {% for fn in self.initialization_fns() -%}
    {{ fn }}();
    {% endfor -%}
})();
