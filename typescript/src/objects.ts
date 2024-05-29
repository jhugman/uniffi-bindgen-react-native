/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import { type FfiConverter, FfiConverterUInt64 } from "./ffi-converters";
import { RustBuffer } from "./ffi-types";
import { UniffiRustArcPtr } from "./rust-call";

/**
 * Marker interface for all `interface` objects that cross the FFI.
 * Reminder: `interface` objects have methods written in Rust.
 *
 * This typesscript interface contains the unffi methods that are needed to make
 * the FFI work. It should shrink to zero methods.
 */
export abstract class AbstractUniffiObject {
  /**
   * Explicitly tell Rust to destroy the native peer that backs this object.
   *
   * Once this method has been called, any following method calls will throw an error.
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
  create(pointer: UnsafeMutableRawPointer): T;
  pointer(obj: T): UnsafeMutableRawPointer;
  clonePointer(obj: T): UnsafeMutableRawPointer;
  freePointer(pointer: UnsafeMutableRawPointer): void;
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
  constructor(private factory: UniffiObjectFactory<T>) {}

  lift(value: UnsafeMutableRawPointer): T {
    return this.factory.create(value);
  }
  lower(value: T): UnsafeMutableRawPointer {
    return this.factory.clonePointer(value);
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

export type FfiConverterObjectWithCallbacks<T> = FfiConverterObject<T>;
