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
