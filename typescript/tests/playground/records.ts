/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import { uniffiCreateRecord } from "../../src/records";

export type MyRecord = {
  string: string;
  number: number;
  optionalString: string | undefined;
  bool: boolean;
  optionalBool: boolean | undefined;
};

export const MyRecord = (() => {
  const defaults = () => ({
    string: "default",
    optionalString: undefined,
  });

  const create = uniffiCreateRecord<MyRecord, ReturnType<typeof defaults>>(
    defaults,
  );

  return {
    create,
  };
})();
