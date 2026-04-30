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
// Hermes (React Native ≥ 0.74) ships TextEncoder and encodeInto, but not
// TextDecoder. For single-string decode (bytesToString), we polyfill via the
// C++ string_from_buffer helper using a duck-typed object matching the
// standard TextDecoder.decode signature. Once Hermes ships a real
// TextDecoder, the `typeof` check will pick it up automatically.
//
// For array-of-strings decode (readStringFromBuffer), we keep a dedicated C++
// helper: the polyfill path (new Uint8Array view + decode) measured ~40%
// slower on getStringArray benchmarks than a direct (buf, offset, length)
// call, due to the per-read view allocation and extra property lookups in
// string_from_buffer.
const stringConverter = (() => {
    const encoder = new TextEncoder();
    const decoder: { decode(input: UniffiByteArray): string } =
        typeof TextDecoder !== "undefined"
            ? new TextDecoder()
            : {
                  decode: (bytes: UniffiByteArray) =>
                      nativeModule().{{ helper.ffi_string_from_buffer }}(
                          bytes,
                          undefined as any,
                      ) as string,
              };
    return {
        // Single-string lower() uses the C++ helper — TextEncoder.encode
        // measured ~43% slower on takeString benchmarks.
        stringToBytes: (s: string) =>
            nativeModule().{{ helper.ffi_string_to_buffer }}(s, undefined as any),
        bytesToString: (ab: UniffiByteArray) => decoder.decode(ab),
        // Direct C++ call — bypasses uniffiCaller.rustCall() overhead.
        // Matters for N-element arrays.
        stringByteLength: (s: string) =>
            nativeModule().{{ helper.ffi_string_to_bytelength }}(s, undefined as any) as number,
        // Encode directly into the RustBuffer backing store via
        // TextEncoder.encodeInto — zero intermediate allocation. Replaces
        // the old C++ write_string_into_buffer helper.
        writeStringIntoBuffer: (s: string, buf: any, offset: number): number => {
            const view = new Uint8Array(
                buf.arrayBuffer,
                offset,
                buf.arrayBuffer.byteLength - offset,
            );
            return encoder.encodeInto(s, view).written;
        },
        // Dedicated C++ helper — avoids per-read Uint8Array allocation and
        // the double property-lookup in string_from_buffer.
        readStringFromBuffer: (buf: any, offset: number, length: number): string =>
            nativeModule().{{ helper.ffi_read_string_from_buffer }}(buf, offset, length) as string,
    };
})();
{%- endif %}
const FfiConverterString = uniffiCreateFfiConverterString(stringConverter);
{%- endmacro %}
