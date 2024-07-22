/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

#include "Bridging.h"
#include "ForeignBytes.h"
#include "UniffiCallInvoker.h"
#include <jsi/jsi.h>

struct RustBuffer {
  size_t capacity;
  size_t len;
  uint8_t *data;
};
