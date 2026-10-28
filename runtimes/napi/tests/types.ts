/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Resolve the public package entry, including the helpers added by lib.js.
import lib, { type FfiTypeDesc, type ResolveLibPathOptions } from "..";

const descriptor: FfiTypeDesc = lib.FfiType.Reference(lib.FfiType.UInt8);
const options: ResolveLibPathOptions = {
  crateName: "fixture",
  callerUrl: import.meta.url,
};
const path: string = lib.resolveLibPath(options);
const module = lib.UniffiNativeModule.open(path);
module.register({ descriptor });
module.unload();
const error: Error = new lib.ResolveLibPathError({
  message: "Missing fixture",
  mode: "colocated",
  crateName: "fixture",
  attempted: [],
});
void error;
// @ts-expect-error References require a descriptor, not an arbitrary string.
lib.FfiType.Reference("UInt8");
// @ts-expect-error The library resolver requires a crate name and caller URL.
lib.resolveLibPath({});
