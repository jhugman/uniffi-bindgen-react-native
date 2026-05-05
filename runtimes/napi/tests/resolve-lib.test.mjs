/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
import { test } from "node:test";
import assert from "node:assert";
import { detectTripleForTesting } from "../typescript/dist/resolve-lib.js";
import {
  resolveLibPath,
  ResolveLibPathError,
  setDetectTripleForTesting,
} from "../typescript/dist/resolve-lib.js";
import {
  mkdirSync,
  mkdtempSync,
  realpathSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

function makeTempDir() {
  const dir = mkdtempSync(join(tmpdir(), "resolve-lib-"));
  return {
    dir,
    cleanup: () => rmSync(dir, { recursive: true, force: true }),
  };
}

// Stub helper: build a fake `process`-like object the detector accepts.
function fakeProcess(platform, arch, { glibc = false } = {}) {
  return {
    platform,
    arch,
    report: {
      getReport() {
        return {
          header: glibc ? { glibcVersionRuntime: "2.31" } : {},
        };
      },
    },
  };
}

test("triple: darwin arm64", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("darwin", "arm64")),
    "darwin-arm64",
  );
});

test("triple: darwin x64", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("darwin", "x64")),
    "darwin-x64",
  );
});

test("triple: linux x64 gnu", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("linux", "x64", { glibc: true })),
    "linux-x64-gnu",
  );
});

test("triple: linux x64 musl", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("linux", "x64", { glibc: false })),
    "linux-x64-musl",
  );
});

test("triple: linux arm64 gnu", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("linux", "arm64", { glibc: true })),
    "linux-arm64-gnu",
  );
});

test("triple: linux arm64 musl", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("linux", "arm64", { glibc: false })),
    "linux-arm64-musl",
  );
});

test("triple: win32 x64", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("win32", "x64")),
    "win32-x64-msvc",
  );
});

test("triple: win32 arm64", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("win32", "arm64")),
    "win32-arm64-msvc",
  );
});

test("triple: unsupported platform throws", () => {
  assert.throws(
    () => detectTripleForTesting(fakeProcess("freebsd", "x64")),
    /Unsupported platform.*freebsd.*x64/,
  );
});

test("triple: unsupported arch throws", () => {
  assert.throws(
    () => detectTripleForTesting(fakeProcess("darwin", "ia32")),
    /Unsupported platform.*darwin.*ia32/,
  );
});

test("override: absolute existing path returns it verbatim", () => {
  const { dir, cleanup } = makeTempDir();
  try {
    const lib = join(dir, "libfoo.dylib");
    writeFileSync(lib, "");
    const got = resolveLibPath({
      crateName: "foo",
      callerUrl: pathToFileURL(join(dir, "caller.js")).toString(),
      override: lib,
    });
    assert.equal(got, lib);
  } finally {
    cleanup();
  }
});

test("override: relative path throws", () => {
  try {
    resolveLibPath({
      crateName: "foo",
      callerUrl: pathToFileURL("/tmp/caller.js").toString(),
      override: "./relative/lib.so",
    });
    assert.fail("expected throw");
  } catch (e) {
    assert.ok(e instanceof ResolveLibPathError);
    assert.equal(e.mode, "override");
    assert.match(e.message, /must be absolute/);
    assert.deepEqual(e.attempted, ["./relative/lib.so"]);
  }
});

test("override: missing absolute path throws", () => {
  try {
    resolveLibPath({
      crateName: "foo",
      callerUrl: pathToFileURL("/tmp/caller.js").toString(),
      override: "/nonexistent/abs/lib.so",
    });
    assert.fail("expected throw");
  } catch (e) {
    assert.ok(e instanceof ResolveLibPathError);
    assert.equal(e.mode, "override");
    assert.match(e.message, /does not exist/);
    assert.deepEqual(e.attempted, ["/nonexistent/abs/lib.so"]);
  }
});

test("colocated: file URL caller resolves to sibling lib", () => {
  const { dir, cleanup } = makeTempDir();
  try {
    const ext =
      process.platform === "darwin"
        ? "dylib"
        : process.platform === "win32"
          ? "dll"
          : "so";
    const fileName =
      process.platform === "win32" ? `foo.${ext}` : `libfoo.${ext}`;
    const lib = join(dir, fileName);
    writeFileSync(lib, "");
    const callerUrl = pathToFileURL(join(dir, "caller.js")).toString();
    const got = resolveLibPath({ crateName: "foo", callerUrl });
    assert.equal(got, lib);
  } finally {
    cleanup();
  }
});

test("colocated: __filename-style raw path works the same", () => {
  const { dir, cleanup } = makeTempDir();
  try {
    const ext =
      process.platform === "darwin"
        ? "dylib"
        : process.platform === "win32"
          ? "dll"
          : "so";
    const fileName =
      process.platform === "win32" ? `foo.${ext}` : `libfoo.${ext}`;
    const lib = join(dir, fileName);
    writeFileSync(lib, "");
    const got = resolveLibPath({
      crateName: "foo",
      callerUrl: join(dir, "caller.js"),
    });
    assert.equal(got, lib);
  } finally {
    cleanup();
  }
});

test("colocated: missing file throws", () => {
  const { dir, cleanup } = makeTempDir();
  try {
    try {
      resolveLibPath({
        crateName: "foo",
        callerUrl: pathToFileURL(join(dir, "caller.js")).toString(),
      });
      assert.fail("expected throw");
    } catch (e) {
      assert.ok(e instanceof ResolveLibPathError);
      assert.equal(e.mode, "colocated");
      assert.match(e.message, /Could not find lib for crate "foo" colocated/);
    }
  } finally {
    cleanup();
  }
});

function buildSyntheticPackage(rootDir, base, triple, libFile) {
  const pkgDir = join(rootDir, "node_modules", `${base}-${triple}`);
  mkdirSync(pkgDir, { recursive: true });
  writeFileSync(
    join(pkgDir, "package.json"),
    JSON.stringify({ name: `${base}-${triple}`, version: "0.0.0" }),
  );
  if (libFile !== null) writeFileSync(join(pkgDir, libFile), "");
  return pkgDir;
}

test("npmPackageBase: package + binary present returns binary path", () => {
  const { dir, cleanup } = makeTempDir();
  const restore = setDetectTripleForTesting(() => "darwin-arm64");
  try {
    const ext =
      process.platform === "darwin"
        ? "dylib"
        : process.platform === "win32"
          ? "dll"
          : "so";
    const libFile =
      process.platform === "win32" ? `foo.${ext}` : `libfoo.${ext}`;
    const pkgDir = buildSyntheticPackage(
      dir,
      "@scope/foo",
      "darwin-arm64",
      libFile,
    );
    const callerUrl = pathToFileURL(join(dir, "caller.js")).toString();
    const got = resolveLibPath({
      crateName: "foo",
      callerUrl,
      npmPackageBase: "@scope/foo",
    });
    assert.equal(got, join(realpathSync(pkgDir), libFile));
  } finally {
    setDetectTripleForTesting(restore);
    cleanup();
  }
});

test("npmPackageBase: package missing throws and names triple + base", () => {
  const { dir, cleanup } = makeTempDir();
  const restore = setDetectTripleForTesting(() => "darwin-arm64");
  try {
    const callerUrl = pathToFileURL(join(dir, "caller.js")).toString();
    try {
      resolveLibPath({
        crateName: "foo",
        callerUrl,
        npmPackageBase: "@scope/foo",
      });
      assert.fail("expected throw");
    } catch (e) {
      assert.ok(e instanceof ResolveLibPathError);
      assert.equal(e.mode, "npmPackageBase");
      assert.match(e.message, /@scope\/foo-darwin-arm64/);
      assert.match(e.message, /darwin-arm64/);
    }
  } finally {
    setDetectTripleForTesting(restore);
    cleanup();
  }
});

test("npmPackageBase: package present but binary missing throws malformed", () => {
  const { dir, cleanup } = makeTempDir();
  const restore = setDetectTripleForTesting(() => "darwin-arm64");
  try {
    buildSyntheticPackage(dir, "@scope/foo", "darwin-arm64", null);
    const callerUrl = pathToFileURL(join(dir, "caller.js")).toString();
    try {
      resolveLibPath({
        crateName: "foo",
        callerUrl,
        npmPackageBase: "@scope/foo",
      });
      assert.fail("expected throw");
    } catch (e) {
      assert.ok(e instanceof ResolveLibPathError);
      assert.equal(e.mode, "npmPackageBase");
      assert.match(e.message, /malformed/);
      assert.equal(e.attempted.length, 1);
    }
  } finally {
    setDetectTripleForTesting(restore);
    cleanup();
  }
});

test("public API: lib.js re-exports resolveLibPath and ResolveLibPathError", async (t) => {
  let lib;
  try {
    lib = (await import("../lib.js")).default;
  } catch {
    // lib.js requires the native NAPI binary which may not be built in
    // TypeScript-only verification environments (e.g. CI without Rust).
    t.skip("native binary not available; skipping lib.js import check");
    return;
  }
  assert.equal(typeof lib.resolveLibPath, "function");
  assert.equal(typeof lib.ResolveLibPathError, "function");
});
