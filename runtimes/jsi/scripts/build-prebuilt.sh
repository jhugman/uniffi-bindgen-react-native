#!/usr/bin/env bash
# Builds the player's Rust half (uniffi-runtime-jsi) as a shared library for
# each cargo target given, or all five shipped slices, into artifacts/<target>/.
# Shared rather than static: a static archive carries the whole std closure
# that the consumer's linker would discard; the dylib is what survives that
# link, about a fortieth of the size.
# Android targets need cargo-ndk and ANDROID_NDK_HOME; Apple targets need Xcode.
set -euo pipefail

HERE=$(cd "$(dirname "$0")/.." && pwd)
ROOT=$(cd "$HERE/../.." && pwd)
OUT="$HERE/artifacts"
# React Native 0.77, the compat floor, requires 15.1; the framework's
# Info.plist says the same.
MIN_IOS_VERSION=15.1

TARGETS=("$@")
if [ ${#TARGETS[@]} -eq 0 ]; then
  TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios aarch64-linux-android x86_64-linux-android)
fi

abi_for() {
  case "$1" in
    aarch64-linux-android) echo arm64-v8a ;;
    x86_64-linux-android)  echo x86_64 ;;
    armv7-linux-androideabi) echo armeabi-v7a ;;
    i686-linux-android)    echo x86 ;;
    *) return 1 ;;
  esac
}

# ndk-host name for the prebuilt llvm-strip path (arch-independent: NDK ships
# one host toolchain, so Apple Silicon and Intel macOS both use x86_64).
ndk_host() {
  case "$(uname -s)" in
    Darwin) echo darwin-x86_64 ;;
    Linux)  echo linux-x86_64 ;;
    *) echo "unsupported host for NDK toolchain: $(uname -s)" >&2; return 1 ;;
  esac
}

# Local symbols and debug info only; the exported ubrn_jsi_* symbols the shim
# links against are untouched.
strip_shared() {
  local t="$1" lib="$2"
  case "$t" in
    *-apple-ios*)
      command -v strip >/dev/null || { echo "strip not found on PATH" >&2; exit 1; }
      strip -x "$lib" ;;
    *-linux-android*)
      : "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME must be set to strip Android libraries}"
      local host; host=$(ndk_host)
      local llvm_strip="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$host/bin/llvm-strip"
      [ -x "$llvm_strip" ] || { echo "llvm-strip not found at $llvm_strip" >&2; exit 1; }
      "$llvm_strip" --strip-unneeded "$lib" ;;
    *)
      echo "unsupported target: $t" >&2; exit 1 ;;
  esac
}

# Checked here rather than at assembly: the link flags above are silent when
# they do not take, and a bad slice otherwise only fails on a device.
verify_shared() {
  local t="$1" lib="$2"
  case "$t" in
    *-linux-android*)
      : "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME must be set to verify Android libraries}"
      local host; host=$(ndk_host)
      local readelf="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$host/bin/llvm-readelf"
      [ -x "$readelf" ] || { echo "llvm-readelf not found at $readelf" >&2; exit 1; }

      local dynamic; dynamic=$("$readelf" -d "$lib")
      case "$dynamic" in
        *"Library soname: [libuniffi_runtime_jsi.so]"*)
          echo "   ok: DT_SONAME is libuniffi_runtime_jsi.so" ;;
        *)
          echo "$dynamic" >&2
          echo "$lib has no DT_SONAME libuniffi_runtime_jsi.so" >&2; exit 1 ;;
      esac

      local headers align
      headers=$("$readelf" -l "$lib")
      align=$(echo "$headers" | awk '$1 == "LOAD" { print $NF; exit }')
      [ "$align" = "0x4000" ] || {
        echo "$headers" >&2
        echo "$lib first LOAD alignment is ${align:-none}, want 0x4000 (16 KB)" >&2; exit 1; }
      echo "   ok: first LOAD segment aligned to $align (16 KB)"

      local exported; exported=$("$readelf" --dyn-syms "$lib" | grep -c 'ubrn_jsi_' || true)
      [ "$exported" -ge 1 ] || {
        "$readelf" --dyn-syms "$lib" >&2
        echo "$lib exports no ubrn_jsi_ symbols" >&2; exit 1; }
      echo "   ok: $exported exported ubrn_jsi_ symbols" ;;
    *-apple-ios*)
      local kind; kind=$(file "$lib")
      case "$kind" in
        *"dynamically linked shared library"*)
          echo "   ok: dynamically linked shared library" ;;
        *)
          echo "$kind" >&2
          echo "$lib is not a dynamically linked shared library" >&2; exit 1 ;;
      esac

      local exported; exported=$(nm -gU "$lib" | grep -c ' T _ubrn_jsi_' || true)
      [ "$exported" -ge 1 ] || {
        nm -gU "$lib" >&2
        echo "$lib exports no ubrn_jsi_ symbols" >&2; exit 1; }
      echo "   ok: $exported exported ubrn_jsi_ symbols" ;;
    *)
      echo "unsupported target: $t" >&2; exit 1 ;;
  esac
}

for t in "${TARGETS[@]}"; do
  echo "-- building uniffi-runtime-jsi for $t"
  case "$t" in
    *-linux-android*)
      # 16 KB page alignment: Android 15 refuses libraries without it. The
      # soname is what a consumer records in DT_NEEDED; without it the linker
      # records the path it was given, which bionic cannot resolve on device.
      (cd "$ROOT" && \
        env "CARGO_TARGET_$(echo "$t" | tr 'a-z-' 'A-Z_')_RUSTFLAGS=-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-soname,libuniffi_runtime_jsi.so" \
        cargo ndk -t "$(abi_for "$t")" build --release -p uniffi-runtime-jsi)
      lib=libuniffi_runtime_jsi.so ;;
    *-apple-ios*)
      (cd "$ROOT" && IPHONEOS_DEPLOYMENT_TARGET="$MIN_IOS_VERSION" cargo build --release --target "$t" -p uniffi-runtime-jsi)
      lib=libuniffi_runtime_jsi.dylib ;;
    *)
      echo "unsupported target: $t" >&2; exit 1 ;;
  esac
  mkdir -p "$OUT/$t"
  cp "$ROOT/target/$t/release/$lib" "$OUT/$t/"
  strip_shared "$t" "$OUT/$t/$lib"
  verify_shared "$t" "$OUT/$t/$lib"
done
echo "-- artifacts:"; ls -la "$OUT"/*/
