import { AbstractFfiConverterByteArray, type FfiConverter } from "./ffi-converters";
import { RustBuffer } from "./ffi-types";
import type { UniffiRustArcPtr } from "./rust-call";
import { type UniffiHandle, UniffiHandleMap } from "./handle-map";
import { UniffiThrownObject } from "./errors";
/**
 * Marker interface for all `interface` objects that cross the FFI.
 * Reminder: `interface` objects have methods written in Rust.
 *
 * This typesscript interface contains the unffi methods that are needed to make
 * the FFI work. It should shrink to zero methods.
 */
export declare abstract class UniffiAbstractObject {
    /**
     * Explicitly tell Rust to destroy the native peer that backs this object.
     *
     * Once this method has been called, any following method calls will throw an error.
     *
     * Can be called more than once.
     */
    abstract uniffiDestroy(): void;
    /**
     * A convenience method to use this object, then destroy it after its use.
     * @param block
     * @returns
     */
    uniffiUse<T>(block: (obj: this) => T): T;
}
/**
 * The JS representation of a Rust pointer.
 */
export type UnsafeMutableRawPointer = bigint;
/**
 * The interface for a helper class generated for each `interface` class.
 *
 * Methods of this interface are not exposed to the API.
 */
export interface UniffiObjectFactory<T> {
    bless(pointer: UnsafeMutableRawPointer): UniffiRustArcPtr;
    unbless(ptr: UniffiRustArcPtr): void;
    create(pointer: UnsafeMutableRawPointer): T;
    pointer(obj: T): UnsafeMutableRawPointer;
    clonePointer(obj: T): UnsafeMutableRawPointer;
    freePointer(pointer: UnsafeMutableRawPointer): void;
    isConcreteType(obj: any): obj is T;
}
/**
 * An FfiConverter for an object.
 */
export declare class FfiConverterObject<T> implements FfiConverter<UnsafeMutableRawPointer, T> {
    private factory;
    constructor(factory: UniffiObjectFactory<T>);
    lift(value: UnsafeMutableRawPointer): T;
    lower(value: T): UnsafeMutableRawPointer;
    read(from: RustBuffer): T;
    write(value: T, into: RustBuffer): void;
    allocationSize(value: T): number;
}
export declare class FfiConverterObjectWithCallbacks<T> extends FfiConverterObject<T> {
    private handleMap;
    constructor(factory: UniffiObjectFactory<T>, handleMap?: UniffiHandleMap<T>);
    lower(value: T): UnsafeMutableRawPointer;
    lift(value: UnsafeMutableRawPointer): T;
    drop(handle: UniffiHandle): T | undefined;
}
export declare class FfiConverterObjectAsError<T> extends AbstractFfiConverterByteArray<UniffiThrownObject<T>> {
    private typeName;
    private innerConverter;
    constructor(typeName: string, innerConverter: FfiConverter<UnsafeMutableRawPointer, T>);
    read(from: RustBuffer): UniffiThrownObject<T>;
    write(value: UniffiThrownObject<T>, into: RustBuffer): void;
    allocationSize(value: UniffiThrownObject<T>): number;
}
