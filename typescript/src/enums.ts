/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
export class UniffiEnum {
  constructor(
    /*
     * This member should be private, but typescript requires
     * it be public because it cannot enforce it.
     */
    public readonly __uniffiTypeName: string,
    /*
     * This member should be private, but typescript requires
     * it be public because it cannot enforce it.
     */
    public readonly __variantName: string,
    /*
     * This member should be private, but typescript requires
     * it be public because it cannot enforce it.
     */
    public readonly __variant: number
  ) {}
}
