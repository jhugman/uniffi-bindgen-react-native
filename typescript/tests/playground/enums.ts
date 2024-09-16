/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { UniffiEnum } from "../../src/enums";

const uniffiTypeNameSymbol = Symbol("uniffiTypeName");
export const MyEnum = (() => {
  const typeName = "MyEnum";
  type Variant1__interface = {
    tag: "Variant1";
    inner: Readonly<{ myValue: string }>;
  };
  class Variant1_ extends UniffiEnum implements Variant1__interface {
    readonly [uniffiTypeNameSymbol]: string = typeName;
    readonly tag = "Variant1";
    readonly inner: Readonly<{ myValue: string }>;
    constructor(inner: { myValue: string }) {
      super(typeName, "Variant1");
      this.inner = Object.freeze(inner);
    }
    static instanceOf(obj: any): obj is Variant1_ {
      return obj.tag === "Variant1" && instanceOf(obj);
    }
  }

  type Variant2__interface = {
    tag: "Variant2";
    inner: Readonly<[number, string]>;
  };
  class Variant2_ extends UniffiEnum implements Variant2__interface {
    readonly [uniffiTypeNameSymbol]: string = typeName;
    readonly tag = "Variant2";
    readonly inner: Readonly<[number, string]>;
    constructor(p1: number, p2: string) {
      super(typeName, "Variant2");
      this.inner = Object.freeze([p1, p2]);
    }
    static instanceOf(obj: any): obj is Variant2_ {
      return obj.tag === "Variant2" && instanceOf(obj);
    }
  }

  function instanceOf(obj: any): obj is MyEnum {
    return obj[uniffiTypeNameSymbol] === "MyEnum";
  }

  return Object.freeze({
    Variant1: Variant1_,
    Variant2: Variant2_,
    instanceOf,
  });
})();

export type MyEnum = InstanceType<
  (typeof MyEnum)[keyof Omit<typeof MyEnum, "instanceOf">]
>;
