/**
 * A destructor guard object is created for every
 * `interface` object.
 *
 * It corresponds to the `DestructibleObject` in C++, which
 * uses a C++ destructor to simulate the JS garbage collector.
 *
 * The implementation is in {@link RustArcPtr.h}.
 */
export declare const destructorGuardSymbol: unique symbol;
/**
 * The `bigint` pointer corresponding to the Rust memory address
 * of the native peer.
 */
export declare const pointerLiteralSymbol: unique symbol;
/**
 * The `string` name of the object, enum or error class.
 *
 * This drives the `instanceOf` method implementations.
 */
export declare const uniffiTypeNameSymbol: unique symbol;
/**
 * The ordinal of the variant in an enum.
 *
 * This is the number that is passed over the FFI.
 */
export declare const variantOrdinalSymbol: unique symbol;
