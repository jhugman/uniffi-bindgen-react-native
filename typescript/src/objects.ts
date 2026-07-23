/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import { Cursor } from "./cursor.ts";
import {
  AbstractFfiConverterByteArray,
  type FfiConverter,
  FfiConverterUInt64,
  type RustBufferAllocator,
} from "./ffi-converters.ts";
import type { UniffiGcObject } from "./rust-call.ts";
import { type UniffiHandle, UniffiHandleMap } from "./handle-map.ts";
import { UniffiThrownObject } from "./errors.ts";

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
 * The interface for a helper class generated for each `interface` class.
 *
 * Methods of this interface are not exposed to the API.
 */
export interface UniffiObjectFactory<T> {
  bless(pointer: UniffiHandle): UniffiGcObject;
  unbless(ptr: UniffiGcObject): void;
  create(pointer: UniffiHandle): T;
  pointer(obj: T): UniffiHandle;
  clonePointer(obj: T): UniffiHandle;
  freePointer(pointer: UniffiHandle): void;
  isConcreteType(obj: any): obj is T;
}

const pointerConverter = FfiConverterUInt64;
const dummyPointer: UniffiHandle = BigInt("0");

/**
 * An FfiConverter for an object.
 */
export class FfiConverterObject<T> implements FfiConverter<UniffiHandle, T> {
  constructor(protected factory: UniffiObjectFactory<T>) {}

  lift(value: UniffiHandle): T {
    return this.factory.create(value);
  }
  lower(value: T, _alloc: RustBufferAllocator): UniffiHandle {
    return this.lowerHandle(value);
  }
  readFromCursor(c: Cursor): T {
    return this.lift(pointerConverter.readFromCursor(c));
  }
  writeIntoCursor(value: T, c: Cursor): void {
    pointerConverter.writeIntoCursor(this.lowerHandle(value), c);
  }
  protected lowerHandle(value: T): UniffiHandle {
    if (this.factory.isConcreteType(value)) {
      return this.factory.clonePointer(value);
    } else {
      throw new Error("Cannot lower this object to a pointer");
    }
  }
  allocationSize(value: T): number {
    return pointerConverter.allocationSize(dummyPointer);
  }
}

/// An FfiConverter for objects with callbacks.
export class FfiConverterObjectWithCallbacks<T> extends FfiConverterObject<T> {
  constructor(
    factory: UniffiObjectFactory<T>,
    private handleMap: UniffiHandleMap<T> = new UniffiHandleMap<T>(),
  ) {
    super(factory);
  }

  lower(value: T, alloc: RustBufferAllocator): UniffiHandle {
    // Rust-backed objects are lowered as raw Arc pointers (even numbers).
    // TS-implemented objects are inserted into the handleMap as foreign handles (odd numbers).
    if (this.factory.isConcreteType(value)) {
      return super.lower(value, alloc);
    }
    return this.handleMap.insert(value);
  }

  lift(value: UniffiHandle): T {
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
    private innerConverter: FfiConverter<UniffiHandle, T>,
  ) {
    super();
  }
  readFromCursor(c: Cursor): UniffiThrownObject<T> {
    const obj = this.innerConverter.readFromCursor(c);
    return new UniffiThrownObject(this.typeName, obj);
  }
  writeIntoCursor(value: UniffiThrownObject<T>, c: Cursor): void {
    const obj = value.inner;
    this.innerConverter.writeIntoCursor(obj, c);
  }
  allocationSize(value: UniffiThrownObject<T>): number {
    return this.innerConverter.allocationSize(value.inner);
  }
}
