{{- self.import_infra("RustBuffer", "ffi-types") }}
{{- self.import_infra_type("UniffiByteArray", "ffi-types") }}
{{- self.import_infra("FfiConverterInt32", "ffi-converters") }}
{{- self.import_infra_type("FfiConverter", "ffi-converters") }}

const stringToArrayBuffer = (s: string): UniffiByteArray =>
    uniffiCaller.rustCall((status) => {% call ts::fn_handle(ci.ffi_function_string_to_arraybuffer()) %}(s, status));

const arrayBufferToString = (ab: UniffiByteArray): string =>
    uniffiCaller.rustCall((status) => {% call ts::fn_handle(ci.ffi_function_arraybuffer_to_string()) %}(ab, status));

const stringByteLength = (s: string): number =>
    uniffiCaller.rustCall((status) => {% call ts::fn_handle(ci.ffi_function_string_to_bytelength()) %}(s, status));

const FfiConverterString = (() => {
    const lengthConverter = FfiConverterInt32;
    type TypeName = string;
    class FFIConverter implements FfiConverter<UniffiByteArray, TypeName> {
        lift(value: UniffiByteArray): TypeName {
            return arrayBufferToString(value);
        }
        lower(value: TypeName): UniffiByteArray {
            return stringToArrayBuffer(value);
        }
        read(from: RustBuffer): TypeName {
            const length = lengthConverter.read(from);
            const bytes = from.readBytes(length);
            return arrayBufferToString(new Uint8Array(bytes));
        }
        write(value: TypeName, into: RustBuffer): void {
            const buffer = stringToArrayBuffer(value).buffer;
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
