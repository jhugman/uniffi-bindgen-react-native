{%- if self.include_once_check("StringHelper.ts") %}
{%- include "RustBufferTemplate.ts" %}
{%- include "Int32Helper.ts" %}
{{- self.add_import_from("UniffiInternalError", "errors") }}
const stringToArrayBuffer = (s: string): ArrayBuffer =>
    rustCall((status) => NativeModule.{{ ci.ffi_function_string_to_arraybuffer().name() }}(s, status));

const arrayBufferToString = (ab: ArrayBuffer): string =>
    rustCall((status) => NativeModule.{{ ci.ffi_function_arraybuffer_to_string().name() }}(ab, status));

const stringByteLength = (s: string): number =>
    rustCall((status) => NativeModule.{{ ci.ffi_function_string_to_bytelength().name() }}(s, status));

const FfiConverterString = (() => {
    const lengthConverter = FfiConverterInt32;
    const byteLength = typeof globalThis.Buffer == 'undefined'
        ? (str: string) => Buffer.byteLength(str, 'utf-8')
        : (str: string) => new Blob([str]).size

    class FFIConverter extends FfiConverterArrayBuffer<string> {
        read(from: RustBuffer): string {
            const length = lengthConverter.read(from);
            return from.read(length, (buffer, offset) => {
                const slice = buffer.slice(offset, offset + length);
                const decoder = new TextDecoder('utf-8');
                return decoder.decode(slice);
            });
        }
        write(value: TypeName, into: RustBuffer): void {
            const length = byteLength(value);
            into.write(length, (buffer, offset) => {
                const encoder = new TextEncoder();
                const view = new Uint8Array(buffer, offset);
                encoder.encodeInto(value, view);
            });
        }
        allocationSize(value: TypeName): number {
            return lengthConverter.allocationSize(0) + byteLength(value);
        }
    }

    return new FFIConverter();
})();
{{- self.add_import_from("initializeWithStringReader", "rust-call") }}
initializeWithStringReader(FfiConverterString.read);
{% endif %}
