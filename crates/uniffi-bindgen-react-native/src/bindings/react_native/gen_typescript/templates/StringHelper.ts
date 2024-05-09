{%- if self.include_once_check("StringHelper.ts") %}
{%- include "RustBufferTemplate.ts" %}
{%- include "Int32Helper.ts" %}
{{- self.add_import_from("UniffiInternalError", "errors") }}
{{- self.add_import_from("FfiConverter", "ffi-converters") }}
const stringToArrayBuffer = (s: string): ArrayBuffer =>
    rustCall((status) => NativeModule.{{ ci.ffi_function_string_to_arraybuffer().name() }}(s, status));

const arrayBufferToString = (ab: ArrayBuffer): string =>
    rustCall((status) => NativeModule.{{ ci.ffi_function_arraybuffer_to_string().name() }}(ab, status));

const stringByteLength = (s: string): number =>
    rustCall((status) => NativeModule.{{ ci.ffi_function_string_to_bytelength().name() }}(s, status));

const FfiConverterString = (() => {
    const lengthConverter = FfiConverterInt32;
    type TypeName = string;
    class FFIConverter implements FfiConverter<ArrayBuffer, TypeName> {
        lift(value: ArrayBuffer): TypeName {
            return arrayBufferToString(value);
        }
        lower(value: TypeName): ArrayBuffer {
            return stringToArrayBuffer(value);
        }
        read(from: RustBuffer): TypeName {
            const length = lengthConverter.read(from);
            return from.read(length, arrayBufferToString);
        }
        write(value: TypeName, into: RustBuffer): void {
            const buffer = stringToArrayBuffer(value);
            const numBytes = buffer.byteLength;
            lengthConverter.write(numBytes, into);
            into.write(numBytes, () => stringToArrayBuffer(value));
        }
        allocationSize(value: TypeName): number {
            return lengthConverter.allocationSize(0) + stringByteLength(value);
        }
    }

    return new FFIConverter();
})();
{{- self.add_import_from("initializeWithStringLifter", "rust-call") }}
initializeWithStringLifter(FfiConverterString.lift);
{% endif %}
