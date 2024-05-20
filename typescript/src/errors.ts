/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
export class UniffiInternalError extends Error {
  private constructor(message: string) {
    super(message);
  }

  static BufferOverflow = class BufferOverflow extends UniffiInternalError {
    constructor() {
      super(
        "Reading the requested value would read past the end of the buffer",
      );
    }
  };
  static IncompleteData = class IncompleteData extends UniffiInternalError {
    constructor() {
      super("The buffer still has data after lifting its containing value");
    }
  };
  static UnexpectedOptionalTag = class UnexpectedOptionalTag extends UniffiInternalError {
    constructor() {
      super("Unexpected optional tag; should be 0 or 1");
    }
  };
  static UnexpectedEnumCase = class UnexpectedEnumCase extends UniffiInternalError {
    constructor() {
      super("Raw enum value doesn't match any cases");
    }
  };
  static UnexpectedNullPointer = class UnexpectedNullPointer extends UniffiInternalError {
    constructor() {
      super("Raw pointer value was null");
    }
  };
  static UnexpectedRustCallStatusCode = class UnexpectedRustCallStatusCode extends UniffiInternalError {
    constructor() {
      super("Unexpected UniffiRustCallStatus code");
    }
  };
  static UnexpectedRustCallError = class UnexpectedRustCallError extends UniffiInternalError {
    constructor() {
      super("CALL_ERROR but no errorClass specified");
    }
  };
  static UnexpectedStaleHandle = class UnexpectedStaleHandle extends UniffiInternalError {
    constructor() {
      super("The object in the handle map has been dropped already");
    }
  };
  static RustPanic = class RustPanic extends UniffiInternalError {
    constructor(message: string) {
      super(message);
    }
  };
  static Unimplemented = class Unimplemented extends UniffiInternalError {
    constructor(message: string) {
      super(message);
    }
  };
}
