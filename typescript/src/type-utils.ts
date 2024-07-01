/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Utility type to ensure two types are structurally the same.
export type StructuralEquality<T, U> = [T] extends [U]
  ? [U] extends [T]
    ? true
    : false
  : false;
