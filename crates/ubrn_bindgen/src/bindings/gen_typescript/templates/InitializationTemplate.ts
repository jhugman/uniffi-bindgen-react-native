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
    const scaffoldingContractVersion = {% call ts::fn_handle(ci.ffi_uniffi_contract_version()) %}();
    if (bindingsContractVersion !== scaffoldingContractVersion) {
        throw new UniffiInternalError.ContractVersionMismatch(scaffoldingContractVersion, bindingsContractVersion);
    }

    {%- for (name, expected_checksum) in ci.iter_checksums() %}
    if ({% call ts::fn_handle_with_name(name) %}() !== {{ expected_checksum }}) {
        throw new UniffiInternalError.ApiChecksumMismatch("{{ name }}");
    }
    {%- endfor %}

    {% for func in self.initialization_fns() -%}
    {{ func }}();
    {% endfor -%}
}
