/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import myModule, {
  AsyncDelegate,
  BasicDelegate,
  getBuilder,
  Builder,
} from "../../generated/uniffi_callback_interface";
import { test } from "@/asserts";
import "@/polyfills";

myModule.initialize();

class MyAsyncDelegate implements AsyncDelegate {
  async method(userId: String) {
    return 'Foo';
  }
};

class MyBasicDelegate implements BasicDelegate {
  method(userId: String) {
    return 'Foo';
  }
};

test("sync delegates", (t) => {
  for (let i = 0; i < 2000; i++) {
    try { 
      const builder = getBuilder();
      const delegate = new MyBasicDelegate();
      console.log(`${i} - ${builder.echo()}`);
      builder.setBasicDelegate(delegate);
      // Allow builder to go out of scope and be reclaimed.
      // (builder as Builder).uniffiDestroy();
    } catch (e) {
      t.fail("Builder should not throw when reclaimed");
    }
  }
});

test("async delegates", (t) => {
  for (let i = 0; i < 2000; i++) {
    try { 
      const builder = getBuilder();
      const delegate = new MyAsyncDelegate();
      console.log(`${i} - ${builder.echo()}`);
      builder.setAsyncDelegate(delegate);
      // Allow builder to go out of scope and be reclaimed.
      // (builder as Builder).uniffiDestroy();
    } catch (e) {
      t.fail("Builder should not throw when reclaimed");
    }
  }
});
