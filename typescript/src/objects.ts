/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Object runtime
export interface UniffiObjectInterface {
  uniffiClonePointer(): UnsafeMutableRawPointer;
  destroy(): void;
}

export type UnsafeMutableRawPointer = bigint;
