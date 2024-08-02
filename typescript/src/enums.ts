/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
export class UniffiEnum {
  constructor(
    private readonly __uniffiTypeName: string,
    private readonly __variantName: string,
    private readonly __variant: number,
  ) {}
}
