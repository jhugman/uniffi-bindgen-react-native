/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { type FfiConverter, FfiConverterUInt64 } from "./ffi-converters";
import { type UniffiByteArray, RustBuffer } from "./ffi-types";
import {
  type UniffiHandle,
  UniffiHandleMap,
  defaultUniffiHandle,
} from "./handle-map";
import {
  CALL_ERROR,
  CALL_UNEXPECTED_ERROR,
  type UniffiRustCallStatus,
} from "./rust-call";

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
  drop(handle: UniffiHandle): T {
    return this.handleMap.remove(handle);
  }
}

export type UniffiReferenceHolder<T> = { pointee: T };

export function uniffiTraitInterfaceCall<T>(
  callStatus: UniffiRustCallStatus,
  makeCall: () => T,
  writeReturn: (v: T) => void,
  lowerString: (s: string) => UniffiByteArray,
) {
  try {
    writeReturn(makeCall());
  } catch (e: any) {
    callStatus.code = CALL_UNEXPECTED_ERROR;
    callStatus.errorBuf = lowerString(e.toString());
  }
}

export function uniffiTraitInterfaceCallWithError<T, E extends Error>(
  callStatus: UniffiRustCallStatus,
  makeCall: () => T,
  writeReturn: (v: T) => void,
  isErrorType: (e: any) => boolean,
  lowerError: (err: E) => UniffiByteArray,
  lowerString: (s: string) => UniffiByteArray,
): void {
  try {
    writeReturn(makeCall());
  } catch (e: any) {
    // Hermes' prototype chain seems buggy, so we need to make our
    // own arrangements
    if (isErrorType(e)) {
      callStatus.code = CALL_ERROR;
      callStatus.errorBuf = lowerError(e as E);
    } else {
      callStatus.code = CALL_UNEXPECTED_ERROR;
      callStatus.errorBuf = lowerString(e.toString());
    }
  }
}
