export type UniffiByteArray = Uint8Array;
export declare class RustBuffer {
    private readOffset;
    private writeOffset;
    private capacity;
    arrayBuffer: ArrayBuffer;
    private constructor();
    static withCapacity(capacity: number): RustBuffer;
    static empty(): RustBuffer;
    static fromArrayBuffer(buf: ArrayBuffer): RustBuffer;
    static fromByteArray(buf: UniffiByteArray): RustBuffer;
    get length(): number;
    get byteArray(): UniffiByteArray;
    readArrayBuffer(numBytes: number): ArrayBuffer;
    readByteArray(numBytes: number): UniffiByteArray;
    writeArrayBuffer(buffer: ArrayBufferLike): void;
    writeByteArray(src: UniffiByteArray): void;
    readWithView<T>(numBytes: number, reader: (view: DataView) => T): T;
    writeWithView(numBytes: number, writer: (view: DataView) => void): void;
    protected checkOverflow(start: number, numBytes: number): number;
}
