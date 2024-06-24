/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { type FfiConverter, FfiConverterUInt64 } from "./ffi-converters";
import { RustBuffer } from "./ffi-types";
import {
  UniffiHandle,
  UniffiHandleMap,
  defaultUniffiHandle,
} from "./handle-map";

const handleConverter = FfiConverterUInt64;

export class FfiConverterCallback<T> implements FfiConverter<UniffiHandle, T> {
  constructor(private handleMap = new UniffiHandleMap<T>()) {}

  lift(value: UniffiHandle): T {
    return this.handleMap.get(value);
  }
  lower(value: T): UniffiHandle {
    return this.handleMap.insert(value);
  }
  read(from: RustBuffer): T {
    return this.lift(handleConverter.read(from));
  }
  write(value: T, into: RustBuffer): void {
    handleConverter.write(this.lower(value), into);
  }
  allocationSize(value: T): number {
    return handleConverter.allocationSize(defaultUniffiHandle);
  }
}
