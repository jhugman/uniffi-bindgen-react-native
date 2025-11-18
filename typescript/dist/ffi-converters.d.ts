import { type UniffiByteArray, RustBuffer } from "./ffi-types";
export interface FfiConverter<FfiType, TsType> {
    lift(value: FfiType): TsType;
    lower(value: TsType): FfiType;
    read(from: RustBuffer): TsType;
    write(value: TsType, into: RustBuffer): void;
    allocationSize(value: TsType): number;
}
export declare abstract class FfiConverterPrimitive<T> implements FfiConverter<T, T> {
    lift(value: T): T;
    lower(value: T): T;
    abstract read(from: RustBuffer): T;
    abstract write(value: T, into: RustBuffer): void;
    abstract allocationSize(value: T): number;
}
export declare abstract class AbstractFfiConverterByteArray<TsType> implements FfiConverter<UniffiByteArray, TsType> {
    lift(value: UniffiByteArray): TsType;
    lower(value: TsType): UniffiByteArray;
    abstract read(from: RustBuffer): TsType;
    abstract write(value: TsType, into: RustBuffer): void;
    abstract allocationSize(value: TsType): number;
}
type NumberType = number | bigint;
declare class FfiConverterNumber<T extends NumberType> extends FfiConverterPrimitive<T> {
    reader: (view: DataView) => T;
    writer: (view: DataView, value: T) => void;
    byteSize: number;
    constructor(reader: (view: DataView) => T, writer: (view: DataView, value: T) => void, byteSize: number);
    read(from: RustBuffer): T;
    write(value: T, into: RustBuffer): void;
    allocationSize(value: T): number;
}
export declare const FfiConverterInt8: FfiConverterNumber<number>;
export declare const FfiConverterInt16: FfiConverterNumber<number>;
export declare const FfiConverterInt32: FfiConverterNumber<number>;
export declare const FfiConverterInt64: FfiConverterNumber<bigint>;
export declare const FfiConverterFloat32: FfiConverterNumber<number>;
export declare const FfiConverterFloat64: FfiConverterNumber<number>;
export declare const FfiConverterUInt8: FfiConverterNumber<number>;
export declare const FfiConverterUInt16: FfiConverterNumber<number>;
export declare const FfiConverterUInt32: FfiConverterNumber<number>;
export declare const FfiConverterUInt64: FfiConverterNumber<bigint>;
export declare const FfiConverterBool: {
    lift(value: number): boolean;
    lower(value: boolean): number;
    read(from: RustBuffer): boolean;
    write(value: boolean, into: RustBuffer): void;
    allocationSize(value: boolean): number;
};
export type UniffiDuration = number;
export declare const FfiConverterDuration: {
    read(from: RustBuffer): UniffiDuration;
    write(value: UniffiDuration, into: RustBuffer): void;
    allocationSize(_value: UniffiDuration): number;
    lift(value: UniffiByteArray): number;
    lower(value: number): UniffiByteArray;
};
export type UniffiTimestamp = Date;
export declare const FfiConverterTimestamp: {
    read(from: RustBuffer): UniffiTimestamp;
    write(value: UniffiTimestamp, into: RustBuffer): void;
    allocationSize(_value: UniffiTimestamp): number;
    lift(value: UniffiByteArray): Date;
    lower(value: Date): UniffiByteArray;
};
export declare class FfiConverterOptional<Item> extends AbstractFfiConverterByteArray<Item | undefined> {
    private itemConverter;
    private static flagConverter;
    constructor(itemConverter: FfiConverter<any, Item>);
    read(from: RustBuffer): Item | undefined;
    write(value: Item | undefined, into: RustBuffer): void;
    allocationSize(value: Item | undefined): number;
}
export declare class FfiConverterArray<Item> extends AbstractFfiConverterByteArray<Array<Item>> {
    private itemConverter;
    private static sizeConverter;
    constructor(itemConverter: FfiConverter<any, Item>);
    read(from: RustBuffer): Array<Item>;
    write(array: Array<Item>, into: RustBuffer): void;
    allocationSize(array: Array<Item>): number;
}
export declare class FfiConverterMap<K, V> extends AbstractFfiConverterByteArray<Map<K, V>> {
    private keyConverter;
    private valueConverter;
    private static sizeConverter;
    constructor(keyConverter: FfiConverter<any, K>, valueConverter: FfiConverter<any, V>);
    read(from: RustBuffer): Map<K, V>;
    write(map: Map<K, V>, into: RustBuffer): void;
    allocationSize(map: Map<K, V>): number;
}
export declare const FfiConverterArrayBuffer: {
    read(from: RustBuffer): ArrayBuffer;
    write(value: ArrayBuffer, into: RustBuffer): void;
    allocationSize(value: ArrayBuffer): number;
    lift(value: UniffiByteArray): ArrayBuffer;
    lower(value: ArrayBuffer): UniffiByteArray;
};
type StringConverter = {
    stringToBytes: (s: string) => UniffiByteArray;
    bytesToString: (ab: UniffiByteArray) => string;
    stringByteLength: (s: string) => number;
};
export declare function uniffiCreateFfiConverterString(converter: StringConverter): FfiConverter<UniffiByteArray, string>;
export {};
