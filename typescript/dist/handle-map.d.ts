export type UniffiHandle = bigint;
export declare const defaultUniffiHandle: bigint;
export declare class UniffiHandleMap<T> {
    private map;
    private currentHandle;
    insert(value: T): UniffiHandle;
    get(handle: UniffiHandle): T;
    remove(handle: UniffiHandle): T | undefined;
    has(handle: UniffiHandle): boolean;
    get size(): number;
}
