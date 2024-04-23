import { RustBuffer } from "./ffi-types";

export interface FfiConverter<FfiType, TsType> {
  lift(value: FfiType): TsType;
  lower(value: TsType): FfiType;
  read(from: RustBuffer): TsType;
  write(value: TsType, into: RustBuffer): void;
  allocationSize(value: TsType): number;
}

export abstract class FfiConverterPrimitive<T> implements FfiConverter<T, T> {
  lift(value: T): T {
    return value;
  }
  lower(value: T): T {
    return value;
  }
  abstract read(from: RustBuffer): T;
  abstract write(value: T, into: RustBuffer): void;
  abstract allocationSize(value: T): number;
}

export abstract class FfiConverterRustBuffer<TsType>
  implements FfiConverter<RustBuffer, TsType>
{
  lift(value: RustBuffer): TsType {
    return this.read(value);
  }
  lower(value: TsType): RustBuffer {
    const buffer = RustBuffer.withCapacity(this.allocationSize(value));
    this.write(value, buffer);
    return buffer;
  }
  abstract read(from: RustBuffer): TsType;
  abstract write(value: TsType, into: RustBuffer): void;
  abstract allocationSize(value: TsType): number;
}

type NumberTypedArray =
  | Int8Array
  | Uint8Array
  | Int16Array
  | Uint16Array
  | Int32Array
  | Uint32Array
  | Float32Array
  | Float64Array;
type BigIntTypeArray = BigInt64Array | BigUint64Array;
type TypedArray = NumberTypedArray | BigIntTypeArray;

type TypedArrayConstructor<T extends TypedArray> = new (
  buffer: ArrayBuffer,
  offset: number,
) => T;
type NumberType = number | bigint;
class FfiConverterNumber<
  T extends NumberType,
  ArrayType extends TypedArray,
> extends FfiConverterPrimitive<T> {
  private constructor(
    private viewConstructor: TypedArrayConstructor<ArrayType>,
    private byteSize: number,
  ) {
    super();
    this.viewConstructor = viewConstructor;
    this.byteSize = byteSize;
  }
  static create<T extends NumberTypedArray>(
    viewConstructor: TypedArrayConstructor<T>,
    byteSize: number,
  ): FfiConverterNumber<number, T> {
    return new FfiConverterNumber<number, T>(viewConstructor, byteSize);
  }
  static create64<T extends BigIntTypeArray>(
    viewConstructor: TypedArrayConstructor<T>,
    byteSize: number,
  ): FfiConverterNumber<bigint, T> {
    return new FfiConverterNumber<bigint, T>(viewConstructor, byteSize);
  }

  read(from: RustBuffer): T {
    return from.read(this.byteSize, (buffer, offset) => {
      const view = new this.viewConstructor(buffer, offset);
      return view.at(0) as T | undefined;
    });
  }

  write(value: T, into: RustBuffer): void {
    into.write(this.byteSize, (buffer, offset) => {
      const view = new this.viewConstructor(buffer, offset);
      view[0] = value;
    });
  }

  allocationSize(value: T): number {
    return this.byteSize;
  }
}

// Ints
export const FfiConverterInt8 = FfiConverterNumber.create(
  Int8Array,
  Int8Array.BYTES_PER_ELEMENT,
);
export const FfiConverterInt16 = FfiConverterNumber.create(
  Int16Array,
  Int16Array.BYTES_PER_ELEMENT,
);
export const FfiConverterInt32 = FfiConverterNumber.create(
  Int32Array,
  Int32Array.BYTES_PER_ELEMENT,
);
export const FfiConverterInt64 = FfiConverterNumber.create64(
  BigInt64Array,
  BigInt64Array.BYTES_PER_ELEMENT,
);

// Floats
export const FfiConverterFloat32 = FfiConverterNumber.create(
  Float32Array,
  Float32Array.BYTES_PER_ELEMENT,
);
export const FfiConverterFloat64 = FfiConverterNumber.create(
  Float64Array,
  Float64Array.BYTES_PER_ELEMENT,
);

// UInts
export const FfiConverterUInt8 = FfiConverterNumber.create(
  Uint8Array,
  Uint8Array.BYTES_PER_ELEMENT,
);
export const FfiConverterUInt16 = FfiConverterNumber.create(
  Uint16Array,
  Uint16Array.BYTES_PER_ELEMENT,
);
export const FfiConverterUInt32 = FfiConverterNumber.create(
  Uint32Array,
  Uint32Array.BYTES_PER_ELEMENT,
);
export const FfiConverterUInt64 = FfiConverterNumber.create64(
  BigUint64Array,
  BigUint64Array.BYTES_PER_ELEMENT,
);

// Bool
export const FfiConverterBool = (() => {
  const byteConverter = FfiConverterInt8;
  class FfiConverterBool implements FfiConverter<number, boolean> {
    lift(value: number): boolean {
      return !!value;
    }
    lower(value: boolean): number {
      return value ? 1 : 0;
    }
    read(from: RustBuffer): boolean {
      return this.lift(byteConverter.read(from));
    }
    write(value: boolean, into: RustBuffer): void {
      byteConverter.write(this.lower(value), into);
    }
    allocationSize(value: boolean): number {
      return byteConverter.allocationSize(0);
    }
  }
  return new FfiConverterBool();
})();

export class FfiConverterOptional<Item> extends FfiConverterRustBuffer<
  Item | undefined
> {
  private static flagConverter = FfiConverterBool;
  constructor(private itemConverter: FfiConverter<any, Item>) {
    super();
  }
  read(from: RustBuffer): Item | undefined {
    const flag = FfiConverterOptional.flagConverter.read(from);
    return flag ? this.itemConverter.read(from) : undefined;
  }
  write(value: Item | undefined, into: RustBuffer): void {
    if (value !== undefined) {
      FfiConverterOptional.flagConverter.write(true, into);
      this.itemConverter.write(value!, into);
    } else {
      FfiConverterOptional.flagConverter.write(false, into);
    }
  }
  allocationSize(value: Item | undefined): number {
    let size = FfiConverterOptional.flagConverter.allocationSize(true);
    if (value !== undefined) {
      size += this.itemConverter.allocationSize(value);
    }
    return size;
  }
}

export class FfiConverterArray<Item> extends FfiConverterRustBuffer<
  Array<Item>
> {
  private static sizeConverter = FfiConverterInt32;
  constructor(private itemConverter: FfiConverter<any, Item>) {
    super();
  }
  read(from: RustBuffer): Array<Item> {
    const size = FfiConverterArray.sizeConverter.read(from);
    const array = new Array<Item>(size);
    for (let i = 0; i < size; i++) {
      array[i] = this.itemConverter.read(from);
    }
    return array;
  }
  write(array: Array<Item>, into: RustBuffer): void {
    FfiConverterArray.sizeConverter.write(array.length, into);
    for (const item of array) {
      this.itemConverter.write(item, into);
    }
  }
  allocationSize(array: Array<Item>): number {
    let size = FfiConverterArray.sizeConverter.allocationSize(array.length);
    for (const item of array) {
      size += this.itemConverter.allocationSize(item);
    }
    return size;
  }
}

export class FfiConverterMap<K, V> extends FfiConverterRustBuffer<Map<K, V>> {
  private static sizeConverter = FfiConverterInt32;
  constructor(
    private keyConverter: FfiConverter<any, K>,
    private valueConverter: FfiConverter<any, V>,
  ) {
    super();
  }
  read(from: RustBuffer): Map<K, V> {
    const size = FfiConverterMap.sizeConverter.read(from);
    const map = new Map();
    for (let i = 0; i < size; i++) {
      map.set(this.keyConverter.read(from), this.valueConverter.read(from));
    }
    return map;
  }
  write(map: Map<K, V>, into: RustBuffer): void {
    FfiConverterMap.sizeConverter.write(map.size, into);
    for (const [k, v] of map.entries()) {
      this.keyConverter.write(k, into);
      this.valueConverter.write(v, into);
    }
  }
  allocationSize(map: Map<K, V>): number {
    let size = FfiConverterMap.sizeConverter.allocationSize(map.size);
    for (const [k, v] of map.entries()) {
      size +=
        this.keyConverter.allocationSize(k) +
        this.valueConverter.allocationSize(v);
    }
    return size;
  }
}
