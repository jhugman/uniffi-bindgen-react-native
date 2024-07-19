/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
constexpr int8_t UNIFFI_CALL_STATUS_OK = 0;
constexpr int8_t UNIFFI_CALL_STATUS_ERROR = 1;
constexpr int8_t UNIFFI_CALL_STATUS_PANIC = 2;

struct RustCallStatus {
  int8_t code;
  RustBuffer error_buf;
};
