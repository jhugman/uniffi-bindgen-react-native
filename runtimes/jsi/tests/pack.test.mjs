/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const pkgDir = join(dirname(fileURLToPath(import.meta.url)), "..");

function packedFiles() {
  const out = execFileSync("npm", ["pack", "--dry-run", "--json"], {
    cwd: pkgDir,
    encoding: "utf8",
  });
  return JSON.parse(out)[0].files.map((f) => f.path);
}

test("the tarball carries what the consumer's build compiles and links", () => {
  const files = packedFiles();
  // What the consumer's iOS and Android builds compile and link.
  for (const must of [
    "package.json",
    "README.md",
    "cpp/shim.cpp",
    "cpp/callbacks.cpp",
    "cpp/abi_assert.cpp",
    "cpp/ubrn_jsi_player.h",
    "include/ubrn_jsi.h",
    "typescript/src/index.ts",
    "typescript/src/NativeUniffiPlayer.ts",
    "typescript/dist/index.js",
    "UbjsReactNative.podspec",
    "Package.swift",
    "ios/UniffiPlayer.h",
    "ios/UniffiPlayer.mm",
    "android/build.gradle",
    "android/gradle.properties",
    "android/CMakeLists.txt",
    "android/cpp-adapter.cpp",
    "android/src/main/AndroidManifest.xml",
    "android/src/main/java/dev/ubjs/reactnative/UniffiPlayerModule.kt",
    "android/src/main/java/dev/ubjs/reactnative/UniffiPlayerPackage.kt",
  ]) {
    assert.ok(files.includes(must), `missing from tarball: ${must}`);
  }
});

test("the tarball carries nothing from the Rust build tree or tests", () => {
  const files = packedFiles();
  for (const f of files) {
    assert.ok(!f.startsWith("src/"), `Rust source should not ship: ${f}`);
    assert.ok(!f.startsWith("tests/"), `tests should not ship: ${f}`);
    assert.ok(!f.startsWith("artifacts/"), `raw artifacts should not ship: ${f}`);
    assert.ok(!f.endsWith("CMakeLists.txt") || f.startsWith("android/"),
      `the host-build CMakeLists must not ship: ${f}`);
  }
});
