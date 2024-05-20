/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
#pragma once

namespace uniffi_jsi {

// Declare the Bridging template
template <typename T> struct Bridging;

/* ArrayBuffer constructor expects MutableBuffer*/
class CMutableBuffer : public jsi::MutableBuffer {
public:
  CMutableBuffer(uint8_t *data, size_t len) : _data(data), len(len) {}
  size_t size() const override { return len; }
  uint8_t *data() override { return _data; }

private:
  uint8_t *_data;
  size_t len;
};

} // namespace uniffi_jsi
