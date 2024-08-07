{{- self.import_infra("UniffiInternalError", "errors") }}
{{- self.import_infra("RustBuffer", "ffi-types") }}
{{- self.import_infra("FfiConverterInt32", "ffi-converters") }}
{{- self.import_infra_type("FfiConverter", "ffi-converters") }}
const stringToArrayBuffer = (s: string): ArrayBuffer =>
    rustCall((status) => nativeModule().{{ ci.ffi_function_string_to_arraybuffer().name() }}(s, status));

const arrayBufferToString = (ab: ArrayBuffer): string =>
    rustCall((status) => nativeModule().{{ ci.ffi_function_arraybuffer_to_string().name() }}(ab, status));

const stringByteLength = (s: string): number =>
    rustCall((status) => nativeModule().{{ ci.ffi_function_string_to_bytelength().name() }}(s, status));

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
            const bytes = from.readBytes(length);
            return arrayBufferToString(bytes);
        }
        write(value: TypeName, into: RustBuffer): void {
            const buffer = stringToArrayBuffer(value);
            const numBytes = buffer.byteLength;
            lengthConverter.write(numBytes, into);
            into.writeBytes(buffer);
        }
        allocationSize(value: TypeName): number {
            return lengthConverter.allocationSize(0) + stringByteLength(value);
        }
    }

    return new FFIConverter();
})();
