/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import { asyncTest } from "@/asserts";
import { console } from "@/hermes";
import {
  ClientBuilder,
  ClientInterface,
  mediaSourceFromUrl,
  MediaSourceInterface,
} from "../../generated/matrix_sdk_ffi";

import { uniffiRustFutureHandleCount } from "uniffi-bindgen-react-native";

const url = "mxc://matrix.1badday.com/RUUIKNjovSSwYbULWuNQarDA";

(async () => {
  await asyncTest("Test for deadlock 2", async (t) => {
    await loadImages(2);
    t.end();
  });

  await asyncTest("Test for deadlock 4", async (t) => {
    await loadImages(4);
    t.end();
  });

  await asyncTest("Test for deadlock 8", async (t) => {
    await loadImages(8);
    t.end();
  });

  await asyncTest("Test for deadlock 16", async (t) => {
    await loadImages(16);
    t.end();
  });

  await asyncTest("Test for deadlock 32", async (t) => {
    await loadImages(32);
    t.end();
  });

  await asyncTest("Test for deadlock 64", async (t) => {
    await loadImages(64);
    t.end();
  });

  await asyncTest("Test for deadlock 128", async (t) => {
    await loadImages(128);
    t.end();
  });

  await asyncTest(
    "Test for deadlock 256",
    async (t) => {
      await loadImages(256);
      t.end();
    },
    30 * 1000,
  );

  await asyncTest(
    "Test for deadlock 512",
    async (t) => {
      await loadImages(512);
      t.end();
    },
    60 * 1000,
  );

  await asyncTest(
    "Test for deadlock 512 2",
    async (t) => {
      await loadImages(512);
      t.end();
    },
    60 * 1000,
  );

  await asyncTest(
    "Test for deadlock 1024",
    async (t) => {
      await loadImages(1024);
      t.end();
    },
    1024 * 1000,
  );
})();

async function loadImages(n: number): Promise<void> {
  const images = new Array(n).fill(url);
  const sourcedImages = images.map((i) => ({
    source: mediaSourceFromUrl(i),
  }));
  const client = await new ClientBuilder()
    .homeserverUrl("https://matrix.1badday.com/")
    .build();
  console.log(`Starting to load… ${n}`);
  let loaded = 0;
  let start = Date.now();
  function elapsed(): number {
    const current = Date.now();
    return Math.round((current - start) / 1000);
  }
  const interval = 1000;
  let progress: any | undefined;
  function show() {
    console.log(
      `-- ${elapsed()} sec: Loaded ${loaded}/${n}; currently waiting on ${uniffiRustFutureHandleCount()}`,
    );
    progress = setTimeout(show, interval);
  }

  const promises = sourcedImages.map((s, i) => {
    return client.getMediaContent(s.source).then((content) => {
      loaded++;
    });
  });
  show();
  await Promise.allSettled(promises);
  clearTimeout(progress!);
  console.log(`… finished, after: ${elapsed()} sec`);
}
