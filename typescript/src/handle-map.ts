/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { UniffiInternalError } from "./errors";

export type UniffiHandle = bigint;
export const defaultUniffiHandle = BigInt("0");

export class UniffiHandleMap<T> {
  private map = new Map<UniffiHandle, T>();
  // Foreign handles must be odd values per uniffi 0.30.x:
  // "Foreign handles are generated with a handle map that only generates odd values."
  // "Foreign handles must always have the lowest bit set"
  // "0 is an invalid value."
  private currentHandle: UniffiHandle = BigInt(1);

  insert(value: T): UniffiHandle {
    const handle = this.currentHandle;
    this.map.set(handle, value);
    this.currentHandle += BigInt(2); // stay odd
    return handle;
  }

  get(handle: UniffiHandle): T {
    const obj = this.map.get(handle);
    if (obj === undefined) {
      // Rust is holding a handle which is no longer in the handle map, either
      // because this is a different handle map to the one it was inserted in,
      // or that the handle has already been removed from the handlemap it was
      // originally in.
      //
      // This is because of either:
      //   a) the Typescript has changed state without resetting a callback
      //      interface, i.e. a hot reload.
      //   b) a bug in uniffi-bindgen-react-native.
      //
      // If this error is thrown when the app is in the wild, i.e. outside of a
      // development, i.e. not a hot reload, then please file a bug with
      // uniffi-bindgen-react-native.
      //
      // Otherwise, this error is not recoverable, and a cold reload is
      // necessary.
      //
      // If the error is not intermittent, i.e. happening every reload, then
      // you should probably consider changing the Rust library to not hold
      // on to callback interfaces and foreign trait instances across reloads,
      // e.g. creating app or page lifecycle API, or replacing rather than
      // appending listeners.
      throw new UniffiInternalError.UnexpectedStaleHandle();
    }
    return obj;
  }

  /**
   * Creates a second handle pointing to the same object. Rust calls this (via
   * `CallbackInterfaceClone`) when it clones an Arc holding a foreign-implemented
   * trait object, so two Arc references can exist with independent lifetimes.
   *
   * This clones the *handle*, not the object itself. Both handles resolve to the
   * same JS object reference, and removing either handle does not affect the other.
   */
  clone(handle: UniffiHandle): UniffiHandle {
    const obj = this.map.get(handle);
    if (obj === undefined) {
      throw new UniffiInternalError.UnexpectedStaleHandle();
    }
    return this.insert(obj);
  }

  remove(handle: UniffiHandle): T | undefined {
    const obj = this.map.get(handle);
    if (obj !== undefined) {
      this.map.delete(handle);
    }
    return obj;
  }

  has(handle: UniffiHandle): boolean {
    return this.map.has(handle);
  }

  get size(): number {
    return this.map.size;
  }
}
