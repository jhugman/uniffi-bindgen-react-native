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
  constructor(
    private readonly __uniffiTypeName: string,
    private readonly __variantName: string,
    private readonly __variant: number,
    message?: string,
  ) {
    super(message);
  }

  static instanceOf(err: any): err is UniffiError {
    return err instanceof Error && (err as any).__uniffiTypeName !== undefined;
  }
}

export const UniffiInternalError = (() => {
  class NumberOverflow extends Error {
    constructor() {
      super("Cannot convert a large BigInt into a number");
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
  class UnexpectedOptionalTag extends Error {
    constructor() {
      super("Unexpected optional tag; should be 0 or 1");
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
    NumberOverflow,
    BufferOverflow,
    IncompleteData,
    UnexpectedOptionalTag,
    UnexpectedEnumCase,
    UnexpectedNullPointer,
    UnexpectedRustCallStatusCode,
    UnexpectedRustCallError,
    UnexpectedStaleHandle,
    RustPanic,
    Unimplemented,
  };
})();
