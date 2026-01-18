/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { UniffiInternalError } from "./errors";

export type UniffiHandle = bigint;
// Initial value and increment amount for handles.
// These ensure that SWIFT handles always have the lowest bit set
const UNIFFI_HANDLEMAP_INITIAL: bigint = BigInt("1");
const UNIFFI_HANDLEMAP_DELTA: bigint = BigInt("2");
export const defaultUniffiHandle = UNIFFI_HANDLEMAP_INITIAL;

export class UniffiHandleMap<T> {
  private map = new Map<UniffiHandle, T>();
  private currentHandle: UniffiHandle = UNIFFI_HANDLEMAP_INITIAL;

  insert(value: T): UniffiHandle {
    return this.doInsert(value);
  }

  private doInsert(value: T) {
    const handle = this.currentHandle;
    this.map.set(this.currentHandle, value);
    this.currentHandle += UNIFFI_HANDLEMAP_DELTA;
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

  clone(handle: UniffiHandle): UniffiHandle {
    const obj = this.map.get(handle);
    if (obj === undefined) {
      // see `get`
      throw new UniffiInternalError.UnexpectedStaleHandle();
    }
    return this.doInsert(obj);
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
