import { type UniffiByteArray } from "./ffi-types";
import { type UniffiReferenceHolder } from "./callbacks";
import { type UniffiRustCallStatus } from "./rust-call";
export type UniffiResult<T> = UniffiReferenceHolder<T> | UniffiRustCallStatus;
export declare const UniffiResult: {
    ready<T>(): UniffiResult<T>;
    writeError<T>(result: UniffiResult<T>, code: number, buf: UniffiByteArray): UniffiResult<T>;
    writeSuccess<T>(result: UniffiResult<T>, obj: T): UniffiResult<T>;
    success<T>(pointee: T): UniffiResult<T>;
};
