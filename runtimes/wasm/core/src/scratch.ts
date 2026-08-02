/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
type Alloc = (size: number) => number;
type Free = (ptr: number, size: number) => void;

/**
 * Stack-discipline bump arena over a region of wasm linear memory.
 *
 * `push(n)` returns a pointer to `n` bytes. If the arena is full, falls
 * back to the supplied allocator. `pop(ptr)` must be called in LIFO order
 * with the value returned by the matching `push`. If the pointer was an
 * overflow allocation, it is freed via the supplied free fn.
 */
export class Scratch {
  private mark = 0; // offset within arena
  private reservedHigh = 0; // boundary between reserved (below) and push/pop (above)
  private hasPushed = false; // once true, no more reserve() calls allowed
  private overflow = new Map<number, number>(); // ptr -> size
  private maxArenaSize: number;

  constructor(
    private arenaPtr: number,
    private arenaSize: number,
    private alloc: Alloc,
    private free: Free,
    maxArenaSize?: number,
  ) {
    this.maxArenaSize = maxArenaSize ?? arenaSize;
  }

  /**
   * Carve a permanent region from the arena, returning its starting offset
   * within wasm memory. Must be called BEFORE any push(). Survives every
   * push/pop cycle for the arena's lifetime.
   *
   * Note: the returned region is a SINGLE address shared across every caller
   * that holds it. It is suitable only for transient scratch within a single
   * synchronous call frame — callers must not assume its contents survive
   * across re-entry. If the wasm export whose dispatcher owns this region is
   * synchronously re-entered (e.g. via a JS callback that calls the same
   * export), the inner call will overwrite the outer call's scratch (status
   * byte, sret slot, lowered arg buffers). Callers that need re-entrant
   * isolation must use `push`/`pop` per call instead of reserving up front.
   */
  reserve(size: number): number {
    if (this.hasPushed) {
      throw new Error("Scratch.reserve: cannot reserve after the first push()");
    }
    // Callers lay RustCallStatus / RustBuffer structs out relative to the
    // returned base assuming it is 8-byte aligned (both have align 8). The
    // arena base is 8-aligned, so keep every carve-off 8-aligned too.
    const start = (this.reservedHigh + 7) & ~7;
    if (start + size > this.arenaSize) {
      throw new Error(
        `Scratch.reserve: arena exhausted (need ${size}, have ${this.arenaSize - start})`,
      );
    }
    const p = this.arenaPtr + start;
    this.reservedHigh = start + size;
    this.mark = this.reservedHigh;
    return p;
  }

  push(size: number): number {
    this.hasPushed = true;
    if (this.mark + size > this.maxArenaSize) {
      const p = this.alloc(size);
      this.overflow.set(p, size);
      return p;
    }
    const p = this.arenaPtr + this.mark;
    this.mark += size;
    return p;
  }

  pop(ptr: number): void {
    const overflowSize = this.overflow.get(ptr);
    if (overflowSize !== undefined) {
      this.overflow.delete(ptr);
      this.free(ptr, overflowSize);
      return;
    }
    // Stack discipline: ptr must be the most recent in-arena push.
    const offset = ptr - this.arenaPtr;
    if (offset < this.reservedHigh || offset >= this.arenaSize) {
      throw new Error(
        `Scratch.pop: pointer ${ptr} not within push region (reserved high=${this.reservedHigh})`,
      );
    }
    this.mark = offset;
  }
}
