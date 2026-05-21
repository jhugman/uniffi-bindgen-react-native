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

test("node triple: darwin arm64", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("darwin", "arm64"), "node"),
    "darwin-arm64",
  );
});

test("node triple: darwin x64", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("darwin", "x64"), "node"),
    "darwin-x64",
  );
});

test("node triple: linux x64 gnu", () => {
  assert.equal(
    detectTripleForTesting(
      fakeProcess("linux", "x64", { glibc: true }),
      "node",
    ),
    "linux-x64-gnu",
  );
});

test("node triple: linux x64 musl", () => {
  assert.equal(
    detectTripleForTesting(
      fakeProcess("linux", "x64", { glibc: false }),
      "node",
    ),
    "linux-x64-musl",
  );
});

test("node triple: linux arm64 gnu", () => {
  assert.equal(
    detectTripleForTesting(
      fakeProcess("linux", "arm64", { glibc: true }),
      "node",
    ),
    "linux-arm64-gnu",
  );
});

test("node triple: linux arm64 musl", () => {
  assert.equal(
    detectTripleForTesting(
      fakeProcess("linux", "arm64", { glibc: false }),
      "node",
    ),
    "linux-arm64-musl",
  );
});

test("node triple: win32 x64", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("win32", "x64"), "node"),
    "win32-x64-msvc",
  );
});

test("node triple: win32 arm64", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("win32", "arm64"), "node"),
    "win32-arm64-msvc",
  );
});

test("node triple: unsupported platform throws", () => {
  assert.throws(
    () => detectTripleForTesting(fakeProcess("freebsd", "x64"), "node"),
    /Unsupported platform.*freebsd.*x64/,
  );
});

test("node triple: unsupported arch throws", () => {
  assert.throws(
    () => detectTripleForTesting(fakeProcess("darwin", "ia32"), "node"),
    /Unsupported platform.*darwin.*ia32/,
  );
});

test("cargo triple: darwin arm64", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("darwin", "arm64"), "cargo"),
    "aarch64-apple-darwin",
  );
});

test("cargo triple: darwin x64", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("darwin", "x64"), "cargo"),
    "x86_64-apple-darwin",
  );
});

test("cargo triple: linux x64 gnu", () => {
  assert.equal(
    detectTripleForTesting(
      fakeProcess("linux", "x64", { glibc: true }),
      "cargo",
    ),
    "x86_64-unknown-linux-gnu",
  );
});

test("cargo triple: linux arm64 musl", () => {
  assert.equal(
    detectTripleForTesting(
      fakeProcess("linux", "arm64", { glibc: false }),
      "cargo",
    ),
    "aarch64-unknown-linux-musl",
  );
});

test("cargo triple: win32 x64", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("win32", "x64"), "cargo"),
    "x86_64-pc-windows-msvc",
  );
});

test("cargo triple: win32 arm64", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("win32", "arm64"), "cargo"),
    "aarch64-pc-windows-msvc",
  );
});

test("cargo triple: unsupported arch throws", () => {
  assert.throws(
    () => detectTripleForTesting(fakeProcess("darwin", "ia32"), "cargo"),
    /Unsupported.*ia32/,
  );
});

test("cargo triple: unsupported platform throws", () => {
  assert.throws(
    () => detectTripleForTesting(fakeProcess("freebsd", "x64"), "cargo"),
    /Unsupported/,
  );
});

test("detectTripleForTesting: defaults to cargo style when style omitted", () => {
  assert.equal(
    detectTripleForTesting(fakeProcess("darwin", "arm64")),
    "aarch64-apple-darwin",
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

/** Build a synthetic npm package at `<rootDir>/node_modules/<pkgName>`. */
function buildSyntheticPackageNamed(rootDir, pkgName, libFile) {
  const pkgDir = join(rootDir, "node_modules", pkgName);
  mkdirSync(pkgDir, { recursive: true });
  writeFileSync(
    join(pkgDir, "package.json"),
    JSON.stringify({ name: pkgName, version: "0.0.0" }),
  );
  if (libFile !== null) writeFileSync(join(pkgDir, libFile), "");
  return pkgDir;
}

function platformLibFile() {
  const ext =
    process.platform === "darwin"
      ? "dylib"
      : process.platform === "win32"
        ? "dll"
        : "so";
  return process.platform === "win32" ? `foo.${ext}` : `libfoo.${ext}`;
}

test("npmPackageBase (cargo): base with trailing '-' joined to cargo triple", () => {
  const { dir, cleanup } = makeTempDir();
  const restore = setDetectTripleForTesting(() => "aarch64-apple-darwin");
  try {
    const libFile = platformLibFile();
    const pkgDir = buildSyntheticPackageNamed(
      dir,
      "@scope/foo-aarch64-apple-darwin",
      libFile,
    );
    const callerUrl = pathToFileURL(join(dir, "caller.js")).toString();
    const got = resolveLibPath({
      crateName: "foo",
      callerUrl,
      npmPackageBase: "@scope/foo-",
      tripleStyle: "cargo",
    });
    assert.equal(got, join(realpathSync(pkgDir), libFile));
  } finally {
    setDetectTripleForTesting(restore);
    cleanup();
  }
});

test("npmPackageBase (node): base with trailing '-' joined to node triple", () => {
  const { dir, cleanup } = makeTempDir();
  const restore = setDetectTripleForTesting(() => "darwin-arm64");
  try {
    const libFile = platformLibFile();
    const pkgDir = buildSyntheticPackageNamed(
      dir,
      "@scope/foo-darwin-arm64",
      libFile,
    );
    const callerUrl = pathToFileURL(join(dir, "caller.js")).toString();
    const got = resolveLibPath({
      crateName: "foo",
      callerUrl,
      npmPackageBase: "@scope/foo-",
      tripleStyle: "node",
    });
    assert.equal(got, join(realpathSync(pkgDir), libFile));
  } finally {
    setDetectTripleForTesting(restore);
    cleanup();
  }
});

test("npmPackageBase: defaults to cargo when tripleStyle omitted", () => {
  const { dir, cleanup } = makeTempDir();
  const restore = setDetectTripleForTesting((style) => {
    assert.equal(style, "cargo", "expected default style 'cargo'");
    return "aarch64-apple-darwin";
  });
  try {
    const libFile = platformLibFile();
    buildSyntheticPackageNamed(
      dir,
      "@scope/foo-aarch64-apple-darwin",
      libFile,
    );
    const callerUrl = pathToFileURL(join(dir, "caller.js")).toString();
    resolveLibPath({
      crateName: "foo",
      callerUrl,
      npmPackageBase: "@scope/foo-",
    });
  } finally {
    setDetectTripleForTesting(restore);
    cleanup();
  }
});

test("npmPackageBase: trailing '/' uses subpath layout (no implicit '-')", () => {
  const { dir, cleanup } = makeTempDir();
  const restore = setDetectTripleForTesting(() => "aarch64-apple-darwin");
  try {
    const libFile = platformLibFile();
    const pkgDir = buildSyntheticPackageNamed(
      dir,
      "@scope/foo/aarch64-apple-darwin",
      libFile,
    );
    const callerUrl = pathToFileURL(join(dir, "caller.js")).toString();
    const got = resolveLibPath({
      crateName: "foo",
      callerUrl,
      npmPackageBase: "@scope/foo/",
      tripleStyle: "cargo",
    });
    assert.equal(got, join(realpathSync(pkgDir), libFile));
  } finally {
    setDetectTripleForTesting(restore);
    cleanup();
  }
});

test("npmPackageBase: package missing throws and names triple + base", () => {
  const { dir, cleanup } = makeTempDir();
  const restore = setDetectTripleForTesting(() => "aarch64-apple-darwin");
  try {
    const callerUrl = pathToFileURL(join(dir, "caller.js")).toString();
    try {
      resolveLibPath({
        crateName: "foo",
        callerUrl,
        npmPackageBase: "@scope/foo-",
        tripleStyle: "cargo",
      });
      assert.fail("expected throw");
    } catch (e) {
      assert.ok(e instanceof ResolveLibPathError);
      assert.equal(e.mode, "npmPackageBase");
      assert.match(e.message, /@scope\/foo-aarch64-apple-darwin/);
      assert.match(e.message, /aarch64-apple-darwin/);
    }
  } finally {
    setDetectTripleForTesting(restore);
    cleanup();
  }
});

test("npmPackageBase: package present but binary missing throws malformed", () => {
  const { dir, cleanup } = makeTempDir();
  const restore = setDetectTripleForTesting(() => "aarch64-apple-darwin");
  try {
    buildSyntheticPackageNamed(dir, "@scope/foo-aarch64-apple-darwin", null);
    const callerUrl = pathToFileURL(join(dir, "caller.js")).toString();
    try {
      resolveLibPath({
        crateName: "foo",
        callerUrl,
        npmPackageBase: "@scope/foo-",
        tripleStyle: "cargo",
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
