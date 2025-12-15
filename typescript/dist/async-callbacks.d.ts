import { type UniffiByteArray } from "./ffi-types";
type UniffiForeignFutureFree = (handle: bigint) => void;
export type UniffiForeignFuture = {
    handle: bigint;
    free: UniffiForeignFutureFree;
};
export declare function uniffiTraitInterfaceCallAsync<T>(makeCall: (signal: AbortSignal) => Promise<T>, handleSuccess: (value: T) => void, handleError: (callStatus: number, errorBuffer: UniffiByteArray) => void, lowerString: (str: string) => UniffiByteArray): UniffiForeignFuture;
export declare function uniffiTraitInterfaceCallAsyncWithError<T, E>(makeCall: (signal: AbortSignal) => Promise<T>, handleSuccess: (value: T) => void, handleError: (callStatus: number, errorBuffer: UniffiByteArray) => void, isErrorType: (error: any) => boolean, lowerError: (error: E) => UniffiByteArray, lowerString: (str: string) => UniffiByteArray): UniffiForeignFuture;
export declare function uniffiForeignFutureHandleCount(): number;
export {};
