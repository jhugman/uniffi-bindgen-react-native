/**
 * This should be called before anything else.
 *
 * It is likely that this is being done for you by the library's `index.ts`.
 *
 * It checks versions of uniffi between when the Rust scaffolding was generated
 * and when the bindings were generated.
 *
 * It also initializes the machinery to enable Rust to talk back to Javascript.
 */
function uniffiEnsureInitialized() {
    {{- self.import_infra("UniffiInternalError", "errors") }}
    // Get the bindings contract version from our ComponentInterface
    const bindingsContractVersion = {{ ci.uniffi_contract_version() }};
    // Get the scaffolding contract version by calling the into the dylib
    const scaffoldingContractVersion = nativeModule().{{ ci.ffi_uniffi_contract_version().name() }}();
    if (bindingsContractVersion !== scaffoldingContractVersion) {
        throw new UniffiInternalError.ContractVersionMismatch(scaffoldingContractVersion, bindingsContractVersion);
    }

    {%- for (name, expected_checksum) in ci.iter_checksums() %}
    if (nativeModule().{{ name }}() !== {{ expected_checksum }}) {
        throw new UniffiInternalError.ApiChecksumMismatch("{{ name }}");
    }
    {%- endfor %}

    {% for fn in self.initialization_fns() -%}
    {{ fn }}();
    {% endfor -%}
}
