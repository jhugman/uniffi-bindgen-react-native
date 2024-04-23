// Object runtime
export interface UniffiObjectInterface {
  uniffiClonePointer(): UnsafeMutableRawPointer;
  destroy(): void;
}

export type UnsafeMutableRawPointer = bigint;
