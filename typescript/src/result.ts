/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import { type UniffiByteArray } from "./ffi-types.js";
import { type UniffiReferenceHolder } from "./callbacks.js";
import { type UniffiRustCallStatus } from "./rust-call.js";

// This Result combines RustCallStatus and ReferenceHolder.
// This is principally so we can _return_ something from calling from native into typescript.

export type UniffiResult<T> = UniffiReferenceHolder<T> | UniffiRustCallStatus;

export const UniffiResult = {
  ready<T>(): UniffiResult<T> {
    return { code: 0 };
  },
  writeError<T>(
    result: UniffiResult<T>,
    code: number,
    buf: UniffiByteArray,
  ): UniffiResult<T> {
    const status = result as UniffiRustCallStatus;
    status.code = code;
    status.errorBuf = buf;
    return status;
  },
  writeSuccess<T>(result: UniffiResult<T>, obj: T): UniffiResult<T> {
    const refHolder = result as UniffiReferenceHolder<T>;
    refHolder.pointee = obj;
    return refHolder;
  },
  success<T>(pointee: T): UniffiResult<T> {
    return { pointee };
  },
};
