/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
export const HANDLE_TAG_SHIFT = 48n;
export const HANDLE_MASK = (1n << HANDLE_TAG_SHIFT) - 1n;

/** Route-by-port marker in the top 16 bits. Rust never reads those bits, so
 * a callback-interface handle comes back to the vtable still tagged. */
export function tagHandle(h: bigint, portId: number): bigint {
  if (h > HANDLE_MASK) {
    throw new Error(
      `message-channel: handle ${h} uses bits above 47, cannot tag`,
    );
  }
  return h | (BigInt(portId) << HANDLE_TAG_SHIFT);
}

export function untagHandle(h: bigint, portId: number): bigint {
  const tag = Number(h >> HANDLE_TAG_SHIFT);
  if (tag !== portId) {
    throw new Error(
      `message-channel: handle ${h} is tagged for port ${tag}, expected port ${portId}`,
    );
  }
  return h & HANDLE_MASK;
}
