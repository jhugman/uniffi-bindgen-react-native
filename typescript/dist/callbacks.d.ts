import { type FfiConverter } from "./ffi-converters";
import { type UniffiByteArray, RustBuffer } from "./ffi-types";
import { type UniffiHandle, UniffiHandleMap } from "./handle-map";
export declare class FfiConverterCallback<T> implements FfiConverter<UniffiHandle, T> {
    private handleMap;
    constructor(handleMap?: UniffiHandleMap<T>);
    lift(value: UniffiHandle): T;
    lower(value: T): UniffiHandle;
    read(from: RustBuffer): T;
    write(value: T, into: RustBuffer): void;
    allocationSize(value: T): number;
    drop(handle: UniffiHandle): T | undefined;
}
export type UniffiReferenceHolder<T> = {
    pointee: T;
};
export declare function uniffiTraitInterfaceCall<T>(makeCall: () => T, handleSuccess: (v: T) => void, handleError: (callStatus: number, errorBuffer: UniffiByteArray) => void, lowerString: (s: string) => UniffiByteArray): void;
export declare function uniffiTraitInterfaceCallWithError<T, E extends Error>(makeCall: () => T, handleSuccess: (v: T) => void, handleError: (callStatus: number, errorBuffer: UniffiByteArray) => void, isErrorType: (e: any) => e is E, lowerError: (err: E) => UniffiByteArray, lowerString: (s: string) => UniffiByteArray): void;
