import { UniffiInternalError } from "./errors";

export class UniffiHandleMap<T> {
  private map = new Map<number, T>();
  private currentHandle: number = 0;

  insert(value: T): number {
    this.map.set(this.currentHandle, value);
    return this.currentHandle++;
  }

  get(handle: number): T {
    const obj = this.map.get(handle);
    if (obj === undefined) {
      throw new UniffiInternalError.UnexpectedStaleHandle();
    }
    return obj;
  }

  remove(handle: number): T {
    const obj = this.map.get(handle);
    if (obj === undefined) {
      throw new UniffiInternalError.UnexpectedStaleHandle();
    }
    this.map.delete(handle);
    return obj;
  }
}
