/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
// Whether `rustbuffer_alloc` views alias Rust memory, so lowering adopts them. False under
// Electron's memory cage, where they are V8 memory marked 0 and lowering copies them.
export function adoptsAllocViews(nm) {
  const view = nm.rustbuffer_alloc(1);
  const [marker] = Object.getOwnPropertySymbols(view);
  const adopts = view[marker] > 0n;
  nm.rustbuffer_free(view);
  return adopts;
}
