/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
const native = require("./index.js");
const {
  resolveLibPath,
  ResolveLibPathError,
} = require("./typescript/dist/resolve-lib.js");

// Required only for a table with async entries, so tables without them work without it.
function asyncEntryBuilder() {
  const { uniffiAsyncEntry } = require("@ubjs/core");
  if (typeof uniffiAsyncEntry !== "function") {
    // Released in lockstep, so this package's version is the oldest core that will do.
    const { version } = require("./package.json");
    throw new Error(
      `@ubjs/node ${version} needs @ubjs/core ${version} or later for async functions`,
    );
  }
  return uniffiAsyncEntry;
}

function asyncFunctions(definitions) {
  const functions = definitions?.functions;
  if (functions === null || typeof functions !== "object") {
    return [];
  }
  return Object.entries(functions).filter(([, def]) => def?.async);
}

// Each async function becomes one entry, `(...args, status, signal) => Promise`, driving the
// future through the exports its `async:` def names. `complete` already returns a JS-owned
// value, so nothing here copies.
const nativeRegister = native.UniffiNativeModule.prototype.register;
native.UniffiNativeModule.prototype.register = function register(definitions) {
  const asyncFns = asyncFunctions(definitions);
  const uniffiAsyncEntry =
    asyncFns.length > 0 ? asyncEntryBuilder() : undefined;
  const nm = nativeRegister.call(this, definitions);
  for (const [name, def] of asyncFns) {
    const { poll, cancel, complete, free } = def.async;
    nm[name] = uniffiAsyncEntry(def.args.length, nm[name], {
      poll: nm[poll],
      cancel: nm[cancel],
      complete: nm[complete],
      free: nm[free],
    });
  }
  return nm;
};

module.exports = {
  ...native,
  resolveLibPath,
  ResolveLibPathError,
  FfiType: {
    UInt8: { tag: "UInt8" },
    Int8: { tag: "Int8" },
    UInt16: { tag: "UInt16" },
    Int16: { tag: "Int16" },
    UInt32: { tag: "UInt32" },
    Int32: { tag: "Int32" },
    UInt64: { tag: "UInt64" },
    Int64: { tag: "Int64" },
    Float32: { tag: "Float32" },
    Float64: { tag: "Float64" },
    Handle: { tag: "Handle" },
    RustBuffer: { tag: "RustBuffer" },
    ForeignBytes: { tag: "ForeignBytes" },
    RustCallStatus: { tag: "RustCallStatus" },
    VoidPointer: { tag: "VoidPointer" },
    Void: { tag: "Void" },
    Callback: (name) => ({ tag: "Callback", name }),
    Struct: (name) => ({ tag: "Struct", name }),
    Reference: (inner) => ({ tag: "Reference", inner }),
    MutReference: (inner) => ({ tag: "MutReference", inner }),
  },
};
