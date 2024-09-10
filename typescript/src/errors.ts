/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */

// The top level error class for all uniffi-wrapped errors.
//
// The readonly fields are used to implement both the instanceOf checks which are used
// in tests and in the generated callback code, and more locally the FfiConverters
// for each error.
export class UniffiError extends Error {
  constructor(enumTypeName: string, variantName: string, message?: string) {
    // We append the error type and variant to the message because we cannot override `toString()`—
    // in errors.test.ts, we see that the overridden `toString()` method is not called.
    super(createErrorMessage(enumTypeName, variantName, message));
  }

  static instanceOf(obj: any): obj is UniffiError {
    return obj.__uniffiTypeName !== undefined && obj instanceof Error;
  }
}

function createErrorMessage(
  typeName: string,
  variantName: string,
  message: string | undefined,
): string {
  const prefix = `${typeName}.${variantName}`;
  if (message) {
    return `${prefix}: ${message}`;
  } else {
    return prefix;
  }
}

export class UniffiThrownObject<T> extends Error {
  private static __baseTypeName = "UniffiThrownObject";
  private readonly __baseTypeName: string = UniffiThrownObject.__baseTypeName;
  constructor(
    typeName: string,
    public readonly inner: T,
    message?: string,
  ) {
    // We append the error type and variant to the message because we cannot override `toString()`—
    // in errors.test.ts, we see that the overridden `toString()` method is not called.
    super(createObjectMessage(typeName, inner, message));
  }

  static instanceOf(err: any): err is UniffiThrownObject<unknown> {
    return (
      !!err &&
      err.__baseTypeName === UniffiThrownObject.__baseTypeName &&
      err instanceof Error
    );
  }
}

function createObjectMessage(
  typeName: string,
  obj: any,
  message: string | undefined,
): string {
  return [typeName, stringRepresentation(obj), message]
    .filter((s) => !!s)
    .join(": ");
}

function stringRepresentation(obj: any): string | undefined {
  if (obj.hasOwnProperty("toString") && typeof obj.toString === "function") {
    return obj.toString();
  }
  if (typeof obj.toDebugString === "function") {
    return obj.toDebugString();
  }
  return undefined;
}

export const UniffiInternalError = (() => {
  class NumberOverflow extends Error {
    constructor() {
      super("Cannot convert a large BigInt into a number");
    }
  }
  class DateTimeOverflow extends Error {
    constructor() {
      super("Date overflowed passed maximum number of ms passed the epoch");
    }
  }
  class BufferOverflow extends Error {
    constructor() {
      super(
        "Reading the requested value would read past the end of the buffer",
      );
    }
  }
  class IncompleteData extends Error {
    constructor() {
      super("The buffer still has data after lifting its containing value");
    }
  }
  class UnexpectedEnumCase extends Error {
    constructor() {
      super("Raw enum value doesn't match any cases");
    }
  }
  class UnexpectedNullPointer extends Error {
    constructor() {
      super("Raw pointer value was null");
    }
  }
  class UnexpectedRustCallStatusCode extends Error {
    constructor() {
      super("Unexpected UniffiRustCallStatus code");
    }
  }
  class UnexpectedRustCallError extends Error {
    constructor() {
      super("CALL_ERROR but no errorClass specified");
    }
  }
  class UnexpectedStaleHandle extends Error {
    constructor() {
      super("The object in the handle map has been dropped already");
    }
  }
  class ContractVersionMismatch extends Error {
    constructor(rustVersion: any, bindingsVersion: any) {
      super(
        `Incompatible versions of uniffi were used to build the JS ($${bindingsVersion}) from the Rust (${rustVersion})`,
      );
    }
  }
  class ApiChecksumMismatch extends Error {
    constructor(func: string) {
      super(
        `FFI function ${func} has a checksum mismatch; this may signify previously undetected incompatible Uniffi versions`,
      );
    }
  }
  class RustPanic extends Error {
    constructor(message: string) {
      super(message);
    }
  }
  class Unimplemented extends Error {
    constructor(message: string) {
      super(message);
    }
  }
  return {
    ApiChecksumMismatch,
    NumberOverflow,
    DateTimeOverflow,
    BufferOverflow,
    ContractVersionMismatch,
    IncompleteData,
    UnexpectedEnumCase,
    UnexpectedNullPointer,
    UnexpectedRustCallStatusCode,
    UnexpectedRustCallError,
    UnexpectedStaleHandle,
    RustPanic,
    Unimplemented,
  };
})();
