/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import {
  AbstractFfiConverterByteArray,
  type FfiConverter,
  FfiConverterUInt64,
} from "./ffi-converters";
import { RustBuffer } from "./ffi-types";
import type { UniffiRustArcPtr } from "./rust-call";
import { type UniffiHandle, UniffiHandleMap } from "./handle-map";
import { type StructuralEquality } from "./type-utils";
import { UniffiInternalError, UniffiThrownObject } from "./errors";

/**
 * Marker interface for all `interface` objects that cross the FFI.
 * Reminder: `interface` objects have methods written in Rust.
 *
 * This typesscript interface contains the unffi methods that are needed to make
 * the FFI work. It should shrink to zero methods.
 */
export abstract class UniffiAbstractObject {
  /**
   * Explicitly tell Rust to destroy the native peer that backs this object.
   *
   * Once this method has been called, any following method calls will throw an error.
   *
   * Can be called more than once.
   */
  public abstract uniffiDestroy(): void;

  /**
   * A convenience method to use this object, then destroy it after its use.
   * @param block
   * @returns
   */
  public uniffiUse<T>(block: (obj: this) => T): T {
    const v = block(this);
    this.uniffiDestroy();
    return v;
  }
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

const pointerConverter: FfiConverter<any, UnsafeMutableRawPointer> =
  FfiConverterUInt64;
const dummyPointer: UnsafeMutableRawPointer = BigInt("0");

/**
 * An FfiConverter for an object.
 */
export class FfiConverterObject<T>
  implements FfiConverter<UnsafeMutableRawPointer, T>
{
  constructor(protected factory: UniffiObjectFactory<T>) {}

  lift(value: UnsafeMutableRawPointer): T {
    return this.factory.create(value);
  }
  lower(value: T): UnsafeMutableRawPointer {
    if (this.factory.isConcreteType(value)) {
      return this.factory.clonePointer(value);
    } else {
      throw new Error("Cannot lower this object to a pointer");
    }
  }
  read(from: RustBuffer): T {
    return this.lift(pointerConverter.read(from));
  }
  write(value: T, into: RustBuffer): void {
    pointerConverter.write(this.lower(value), into);
  }
  allocationSize(value: T): number {
    return pointerConverter.allocationSize(dummyPointer);
  }
}

/// An FfiConverter for objects with callbacks.
const handleSafe: StructuralEquality<UniffiHandle, UnsafeMutableRawPointer> =
  true;

export class FfiConverterObjectWithCallbacks<T> extends FfiConverterObject<T> {
  constructor(
    factory: UniffiObjectFactory<T>,
    private handleMap: UniffiHandleMap<T> = new UniffiHandleMap<T>(),
  ) {
    super(factory);
  }

  lower(value: T): UnsafeMutableRawPointer {
    // Rust-backed objects are lowered as raw Arc pointers (even numbers).
    // TS-implemented objects are inserted into the handleMap as foreign handles (odd numbers).
    if (this.factory.isConcreteType(value)) {
      return super.lower(value);
    }
    return this.handleMap.insert(value);
  }

  lift(value: UnsafeMutableRawPointer): T {
    if (this.handleMap.has(value)) {
      return this.handleMap.get(value);
    } else {
      return super.lift(value);
    }
  }

  drop(handle: UniffiHandle): T | undefined {
    return this.handleMap.remove(handle);
  }

  /**
   * Called by Rust's `CallbackInterfaceClone` vtable entry when it clones an
   * Arc holding a foreign-implemented object. Returns a new handle pointing to
   * the same JS object; removing either handle does not affect the other.
   */
  clone(handle: UniffiHandle): UniffiHandle {
    return this.handleMap.clone(handle);
  }
}

/// Due to some mismatches in the ffi converter mechanisms, errors are a RustBuffer holding a pointer
export class FfiConverterObjectAsError<T> extends AbstractFfiConverterByteArray<
  UniffiThrownObject<T>
> {
  constructor(
    private typeName: string,
    private innerConverter: FfiConverter<UnsafeMutableRawPointer, T>,
  ) {
    super();
  }
  read(from: RustBuffer): UniffiThrownObject<T> {
    const obj = this.innerConverter.read(from);
    return new UniffiThrownObject(this.typeName, obj);
  }
  write(value: UniffiThrownObject<T>, into: RustBuffer): void {
    const obj = value.inner;
    this.innerConverter.write(obj, into);
  }
  allocationSize(value: UniffiThrownObject<T>): number {
    return this.innerConverter.allocationSize(value.inner);
  }
}
