/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { UniffiInternalError } from "./errors";

export class UniffiHandleMap<T> {
  private map = new Map<number, T>();
  private currentHandle: number = 0;

  insert(value: T): number {
    this.map.set(this.currentHandle, value);
    return this.currentHandle++;
  }

  get(handle: number): T {
    const obj = this.map.get(handle);
    if (obj === undefined) {
      throw new UniffiInternalError.UnexpectedStaleHandle();
    }
    return obj;
  }

  remove(handle: number): T {
    const obj = this.map.get(handle);
    if (obj === undefined) {
      throw new UniffiInternalError.UnexpectedStaleHandle();
    }
    this.map.delete(handle);
    return obj;
  }
}
