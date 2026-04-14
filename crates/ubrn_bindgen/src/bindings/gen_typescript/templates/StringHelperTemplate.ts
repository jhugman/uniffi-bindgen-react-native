{%- macro string_helper(helper) %}
{%- if helper.supports_text_encoder %}
const stringConverter = (() => {
    const encoder = new TextEncoder();
    const decoder = new TextDecoder();
    return {
        stringToBytes: (s: string) => encoder.encode(s),
        bytesToString: (ab: UniffiByteArray) => decoder.decode(ab),
        stringByteLength: (s: string) => encoder.encode(s).byteLength,
    };
})();
{%- else %}
const stringConverter = {
    stringToBytes: (s: string) =>
        uniffiCaller.rustCall((status) => nativeModule().{{ helper.ffi_string_to_arraybuffer }}(s, status)),
    bytesToString: (ab: UniffiByteArray) =>
        uniffiCaller.rustCall((status) => nativeModule().{{ helper.ffi_arraybuffer_to_string }}(ab, status)),
    stringByteLength: (s: string) =>
        uniffiCaller.rustCall((status) => nativeModule().{{ helper.ffi_string_to_bytelength }}(s, status)),
};
{%- endif %}
const FfiConverterString = uniffiCreateFfiConverterString(stringConverter);
{%- endmacro %}
