{{- self.import_infra_type("UniffiByteArray", "ffi-types") }}
{{- self.import_infra("uniffiCreateFfiConverterString", "ffi-converters") }}

{%- if flavor.supports_text_encoder() %}
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
        uniffiCaller.rustCall((status) => {% call ts::fn_handle(ci.ffi_function_string_to_arraybuffer()) %}(s, status)),
    bytesToString: (ab: UniffiByteArray) =>
        uniffiCaller.rustCall((status) => {% call ts::fn_handle(ci.ffi_function_arraybuffer_to_string()) %}(ab, status)),
    stringByteLength: (s: string) =>
        uniffiCaller.rustCall((status) => {% call ts::fn_handle(ci.ffi_function_string_to_bytelength()) %}(s, status)),
};
{%- endif %}
const FfiConverterString = uniffiCreateFfiConverterString(stringConverter);
