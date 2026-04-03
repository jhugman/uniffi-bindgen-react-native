{%- macro initialization(init) %}
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
    // Get the bindings contract version from our ComponentInterface
    const bindingsContractVersion = {{ init.bindings_contract_version }};
    // Get the scaffolding contract version by calling the into the dylib
    const scaffoldingContractVersion = nativeModule().{{ init.ffi_contract_version_fn }}();
    if (bindingsContractVersion !== scaffoldingContractVersion) {
        throw new UniffiInternalError.ContractVersionMismatch(scaffoldingContractVersion, bindingsContractVersion);
    }

    {%- for checksum in init.checksums %}
    if (nativeModule().{{ checksum.ffi_fn_name }}() !== {{ checksum.expected_value }}) {
        throw new UniffiInternalError.ApiChecksumMismatch("{{ checksum.raw_name }}");
    }
    {%- endfor %}

    {% for func in init.initialization_fns -%}
    {{ func }}();
    {% endfor -%}
}
{%- endmacro %}
