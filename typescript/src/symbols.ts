/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// Symbols for semi-private properties. These properties should
// not be visible to users.
//
// The documentation refers to the property itself, of
// which these symbols are the property name.

/**
 * A destructor guard object is created for every
 * `interface` object.
 *
 * It corresponds to the `DestructibleObject` in C++, which
 * uses a C++ destructor to simulate the JS garbage collector.
 *
 * The implementation is in {@link RustArcPtr.h}.
 */
export const destructorGuardSymbol = Symbol.for("destructor");

/**
 * The `bigint` pointer corresponding to the Rust memory address
 * of the native peer.
 */
export const pointerLiteralSymbol = Symbol.for("pointer");

/**
 * The `string` name of the object, enum or error class.
 *
 * This drives the `instanceOf` method implementations.
 */
export const uniffiTypeNameSymbol = Symbol.for("typeName");

/**
 * The ordinal of the variant in an enum.
 *
 * This is the number that is passed over the FFI.
 */
export const variantOrdinalSymbol = Symbol.for("variant");
