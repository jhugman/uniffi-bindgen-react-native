/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, isAbsolute, join } from "node:path";
import { fileURLToPath } from "node:url";

interface ProcessLike {
  platform: NodeJS.Platform;
  arch: string;
  report?: { getReport(): { header?: { glibcVersionRuntime?: string } } };
}

function detectTriple(proc: ProcessLike): string {
  const { platform, arch } = proc;
  if (platform === "darwin" && arch === "arm64") return "darwin-arm64";
  if (platform === "darwin" && arch === "x64") return "darwin-x64";
  if (platform === "linux" && (arch === "x64" || arch === "arm64")) {
    const isGnu =
      proc.report?.getReport()?.header?.glibcVersionRuntime !== undefined;
    return `linux-${arch}-${isGnu ? "gnu" : "musl"}`;
  }
  if (platform === "win32" && (arch === "x64" || arch === "arm64")) {
    return `win32-${arch}-msvc`;
  }
  throw new Error(
    `Unsupported platform/arch combination: ${platform}/${arch}. ` +
      `Supported: darwin-{arm64,x64}, linux-{x64,arm64}-{gnu,musl}, win32-{x64,arm64}-msvc.`,
  );
}

/** Test-only export. Do not use from production code. */
export function detectTripleForTesting(proc: ProcessLike): string {
  return detectTriple(proc);
}

export type ResolveMode = "override" | "npmPackageBase" | "colocated";

export class ResolveLibPathError extends Error {
  readonly mode: ResolveMode;
  readonly crateName: string;
  readonly attempted: string[];
  override readonly cause?: unknown;

  constructor(args: {
    message: string;
    mode: ResolveMode;
    crateName: string;
    attempted: string[];
    cause?: unknown;
  }) {
    super(args.message);
    this.name = "ResolveLibPathError";
    this.mode = args.mode;
    this.crateName = args.crateName;
    this.attempted = args.attempted;
    if (args.cause !== undefined) this.cause = args.cause;
  }
}

export type ResolveLibPathOptions = {
  crateName: string;
  callerUrl: string;
} & (
  | { override: string; npmPackageBase?: never }
  | { npmPackageBase: string; override?: never }
  | { override?: never; npmPackageBase?: never }
);

function callerDir(callerUrl: string): string {
  const path = callerUrl.startsWith("file://")
    ? fileURLToPath(callerUrl)
    : callerUrl;
  return dirname(path);
}

function libFileName(crateName: string, platform: NodeJS.Platform): string {
  if (platform === "win32") return `${crateName}.dll`;
  const ext = platform === "darwin" ? "dylib" : "so";
  return `lib${crateName}.${ext}`;
}

export function resolveLibPath(opts: ResolveLibPathOptions): string {
  if ("override" in opts && opts.override !== undefined) {
    return resolveOverride(opts.crateName, opts.override);
  }
  if ("npmPackageBase" in opts && opts.npmPackageBase !== undefined) {
    return resolveNpmPackage(
      opts.crateName,
      opts.callerUrl,
      opts.npmPackageBase,
    );
  }
  return resolveColocated(opts.crateName, opts.callerUrl);
}

function resolveColocated(crateName: string, callerUrl: string): string {
  const candidate = join(
    callerDir(callerUrl),
    libFileName(crateName, process.platform),
  );
  if (!existsSync(candidate)) {
    throw new ResolveLibPathError({
      message: `Could not find lib for crate "${crateName}" colocated with caller. Looked for: ${candidate}.`,
      mode: "colocated",
      crateName,
      attempted: [candidate],
    });
  }
  return candidate;
}

function resolveOverride(crateName: string, path: string): string {
  if (!isAbsolute(path)) {
    throw new ResolveLibPathError({
      message: `Override path must be absolute; got "${path}".`,
      mode: "override",
      crateName,
      attempted: [path],
    });
  }
  if (!existsSync(path)) {
    throw new ResolveLibPathError({
      message: `Override path "${path}" for crate "${crateName}" does not exist.`,
      mode: "override",
      crateName,
      attempted: [path],
    });
  }
  return path;
}

let _detectTripleImpl: () => string = () =>
  detectTriple(process as ProcessLike);

/** Test-only seam to override triple detection. Returns the previous impl. */
export function setDetectTripleForTesting(fn: () => string): () => string {
  const prev = _detectTripleImpl;
  _detectTripleImpl = fn;
  return prev;
}

function resolveNpmPackage(
  crateName: string,
  callerUrl: string,
  npmPackageBase: string,
): string {
  const triple = _detectTripleImpl();
  const pkgName = `${npmPackageBase}-${triple}`;
  const require_ = createRequire(callerUrl);

  let pkgJsonPath: string;
  try {
    pkgJsonPath = require_.resolve(`${pkgName}/package.json`);
  } catch (cause) {
    throw new ResolveLibPathError({
      message:
        `Could not find platform package for crate "${crateName}": tried "${pkgName}". ` +
        `Detected platform: ${triple}. Either the package is not installed (run npm install) ` +
        `or this platform is not in the published matrix.`,
      mode: "npmPackageBase",
      crateName,
      attempted: [pkgName],
      cause,
    });
  }

  const binaryPath = join(
    dirname(pkgJsonPath),
    libFileName(crateName, process.platform),
  );
  if (!existsSync(binaryPath)) {
    throw new ResolveLibPathError({
      message:
        `Platform package "${pkgName}" is installed but does not contain ` +
        `"${libFileName(crateName, process.platform)}" — package layout is malformed.`,
      mode: "npmPackageBase",
      crateName,
      attempted: [binaryPath],
    });
  }
  return binaryPath;
}
