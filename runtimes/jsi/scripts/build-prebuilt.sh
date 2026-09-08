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

# compiler-rt's builtins for an Android ABI. libffi's per-arch trampoline code
# calls __clear_cache, a compiler-rt builtin that bionic does not carry and
# Rust's compiler_builtins does not supply; clang adds this archive when it
# drives the link, rustc does not.
builtins_archive() {
  local t="$1" host arch
  host=$(ndk_host)
  case "$t" in
    aarch64-linux-android)   arch=aarch64 ;;
    x86_64-linux-android)    arch=x86_64 ;;
    armv7-linux-androideabi) arch=arm ;;
    i686-linux-android)      arch=i686 ;;
    *) echo "unsupported target: $t" >&2; return 1 ;;
  esac
  # The clang version directory moves with the NDK, so glob for it.
  local found=("$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$host/lib/clang"/*/lib/linux/"libclang_rt.builtins-$arch-android.a")
  [ -e "${found[0]}" ] || {
    echo "libclang_rt.builtins-$arch-android.a not found under $ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$host/lib/clang/*/lib/linux/" >&2
    return 1; }
  echo "${found[0]}"
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
      echo "   ok: $exported exported ubrn_jsi_ symbols"

      # libffi is linked statically and compiler-rt explicitly, so neither may
      # be left undefined: a shared object links happily with either missing
      # and only dlopen on device rejects it. --dynamic because the check runs
      # after the strip, which leaves only .dynsym -- the table dlopen reads.
      local nm; nm="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$host/bin/llvm-nm"
      [ -x "$nm" ] || { echo "llvm-nm not found at $nm" >&2; exit 1; }
      local unresolved
      unresolved=$("$nm" --dynamic --undefined-only --format=just-symbols "$lib" | grep -E '^(ffi_|__clear_cache)' || true)
      [ -z "$unresolved" ] || {
        echo "$unresolved" >&2
        echo "$lib leaves libffi/compiler-rt symbols undefined; dlopen will reject it" >&2; exit 1; }
      echo "   ok: no undefined ffi_/__clear_cache symbols" ;;
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
      : "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME must be set to build Android libraries}"
      ndk_bin="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$(ndk_host)/bin"
      # libffi-sys builds libffi from source with autotools, whose configure
      # falls back to the host archiver when it finds no target-prefixed one.
      # Apple's ar writes an archive with no members from the ELF objects, so
      # libffi.a comes out empty and every ffi_* symbol is left undefined. Set
      # the plain and the target-suffixed forms both, so cargo-ndk's own
      # target-suffixed settings cannot shadow them.
      target_env=$(echo "$t" | tr '-' '_')
      builtins=$(builtins_archive "$t")
      # 16 KB page alignment: Android 15 refuses libraries without it. The
      # soname is what a consumer records in DT_NEEDED; without it the linker
      # records the path it was given, which bionic cannot resolve on device.
      (cd "$ROOT" && \
        env \
        AR="$ndk_bin/llvm-ar" RANLIB="$ndk_bin/llvm-ranlib" \
        "AR_$target_env=$ndk_bin/llvm-ar" "RANLIB_$target_env=$ndk_bin/llvm-ranlib" \
        "CARGO_TARGET_$(echo "$t" | tr 'a-z-' 'A-Z_')_RUSTFLAGS=-C link-arg=-Wl,-z,max-page-size=16384 -C link-arg=-Wl,-soname,libuniffi_runtime_jsi.so -C link-arg=$builtins" \
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
