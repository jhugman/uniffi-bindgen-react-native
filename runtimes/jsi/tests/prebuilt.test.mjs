/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const pkgDir = join(dirname(fileURLToPath(import.meta.url)), "..");

const frameworkExecutables = [
  "ios/UniffiRuntimeJsi.xcframework/ios-arm64/UniffiRuntimeJsi.framework/UniffiRuntimeJsi",
  "ios/UniffiRuntimeJsi.xcframework/ios-arm64_x86_64-simulator/UniffiRuntimeJsi.framework/UniffiRuntimeJsi",
];

// Separate from pack.test.mjs: the compile gate and local dev builds only
// build the slices they need, so prebuilt/ is complete only for the release
// assembly step. This runs on its own, not as part of `npm test`.
test("prebuilt/ carries every slice the publish tarball ships", () => {
  for (const slice of [
    "android/arm64-v8a/libuniffi_runtime_jsi.so",
    "android/x86_64/libuniffi_runtime_jsi.so",
    "ios/UniffiRuntimeJsi.xcframework/Info.plist",
    ...frameworkExecutables,
    "ios/UniffiRuntimeJsi.xcframework/ios-arm64/UniffiRuntimeJsi.framework/Info.plist",
    "ios/UniffiRuntimeJsi.xcframework/ios-arm64_x86_64-simulator/UniffiRuntimeJsi.framework/Info.plist",
  ]) {
    const path = join(pkgDir, "prebuilt", slice);
    assert.ok(existsSync(path), `missing from prebuilt/: ${slice}`);
  }
});

// otool and file are macOS-only, as is the publish job that assembles the
// xcframework. A missing slice is the existence test's business, so skip it
// here rather than failing twice.
const onMacOS = process.platform === "darwin" ? test : test.skip;
onMacOS("each framework slice is a dylib the app loads by @rpath", async (t) => {
  for (const slice of frameworkExecutables) {
    const path = join(pkgDir, "prebuilt", slice);
    await t.test(slice, (sub) => {
      if (!existsSync(path)) {
        sub.skip(`not present in prebuilt/: ${slice}`);
        return;
      }
      const installName = execFileSync("otool", ["-D", path], {
        encoding: "utf8",
      });
      assert.ok(
        installName.includes("@rpath/UniffiRuntimeJsi.framework/UniffiRuntimeJsi"),
        `install name is not @rpath-relative:\n${installName}`,
      );
      const kind = execFileSync("file", [path], { encoding: "utf8" });
      assert.ok(
        kind.includes("dynamically linked shared library"),
        `not a dynamic library:\n${kind}`,
      );
    });
  }
});
