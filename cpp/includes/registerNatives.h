/*
 * This is free and unencumbered software released into the public domain.
 * For more information, please refer to the LICENSE file in the root directory
 * or at <http://unlicense.org/>
 */

#pragma once

#include <hermes/hermes.h>

/// Register host functions into the given runtime.
extern "C" void registerNatives(facebook::jsi::Runtime &rt);
