{%- let ns = self.config.project.cpp_namespace() %}
{%- let marker = ns|upper|fmt("{}_H") -%}
#ifndef {{ marker }}
#define {{ marker }}
// Generated by uniffi-bindgen-react-native
#include <cstdint>
#include <jsi/jsi.h>
#include <ReactCommon/CallInvoker.h>

namespace {{ ns }} {
  using namespace facebook;

  // TODO Remove `multiply` after seeing this work on iOS and Android.
  double multiply(double a, double b);

  uint8_t installRustCrate(jsi::Runtime &runtime, uint8_t b);
  uint8_t cleanupRustCrate(jsi::Runtime &runtime, uint8_t b);
}

#endif /* {{ marker }} */