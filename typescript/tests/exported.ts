/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

export type MyCustomString = string;

export const MyEnum = (() => {
  return {
    Variant1: class {},
    Variant2: class {},
    instanceOf: (obj: any): boolean => true,
  };
})();
export type MyEnum = InstanceType<
  (typeof MyEnum)[keyof Omit<typeof MyEnum, "instanceOf">]
>;

export type MyRecord = {
  prop1: string;
  prop2: number;
};

export interface MyCallbackInterface {
  myMethod(): void;
}

export interface MyObjectInterface {
  myForeignMethod(): void;
}
export class MyObject implements MyObjectInterface {
  myForeignMethod(): void {}
}
