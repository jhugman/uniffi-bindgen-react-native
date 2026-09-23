#!/usr/bin/env bash
# Compile gate for @ubjs/react-native: scaffold a plain React Native app, add
# the packed player as a DIRECT dependency (autolinking reads only the app's
# package.json), and build it for the iOS simulator or Android.
set -euo pipefail

usage() {
  echo "Usage: $0 (--ios | --android) [--run] [--rn-version VERSION] [--work-dir DIR] [--keep]"
}

PLATFORM=""; RN_VERSION="latest"; WORK="/tmp/ubrn-player-smoke"; KEEP=false; RUN=false
while [ $# -gt 0 ]; do
  case "$1" in
    --ios) PLATFORM=ios ;;
    --android) PLATFORM=android ;;
    --run) RUN=true ;;
    --rn-version) RN_VERSION="$2"; shift ;;
    --work-dir) WORK="$2"; shift ;;
    --keep) KEEP=true ;;
    -h|--help) usage; exit 0 ;;
    *) usage; exit 1 ;;
  esac
  shift
done
[ -n "$PLATFORM" ] || { usage; exit 1; }
# One subdirectory per platform: both platforms into one work dir would wipe
# whatever --keep held on to from the earlier run.
WORK="$WORK/$PLATFORM"

ROOT=$(git rev-parse --show-toplevel)
# shellcheck source=scripts/lib/rn-app.sh
source "$ROOT/scripts/lib/rn-app.sh"
trap rnapp_cleanup EXIT
APP=PlayerSmoke

echo "-- 1. prebuilt Rust half for $PLATFORM"
if [ "$PLATFORM" = ios ]; then
  # CocoaPods picks the one xcframework slice covering every arch in ARCHS, and
  # the generic simulator destination asks for both, so ship both here too.
  "$ROOT/runtimes/jsi/scripts/build-prebuilt.sh" aarch64-apple-ios-sim x86_64-apple-ios
else
  "$ROOT/runtimes/jsi/scripts/build-prebuilt.sh" aarch64-linux-android x86_64-linux-android
fi
"$ROOT/runtimes/jsi/scripts/assemble-prebuilt.sh"

echo "-- 2. pack @ubjs/core and @ubjs/react-native"
(cd "$ROOT/typescript" && npm install && npm run build)
CORE_TGZ=$(cd "$ROOT/typescript" && npm pack --silent | tail -1)
(cd "$ROOT/runtimes/jsi" && npm install && npm run build)
PLAYER_TGZ=$(cd "$ROOT/runtimes/jsi" && npm pack --silent | tail -1)

echo "-- 3. scaffold a React Native $RN_VERSION app"
rm -rf "$WORK"; mkdir -p "$WORK"
npx --yes @react-native-community/cli@latest init "$APP" \
  --version "$RN_VERSION" --directory "$WORK/$APP" --skip-install --skip-git-init --pm npm
cd "$WORK/$APP"
npm install
npm install "$ROOT/typescript/$CORE_TGZ" "$ROOT/runtimes/jsi/$PLAYER_TGZ"
# Importing the player installs it; the sentinel proves the TurboModule
# registered and put the host object on the global.
# shellcheck disable=SC2016  # ${...} here is a JS template literal, not shell.
rnapp_prepend_app_tsx "$WORK/$APP" 'import "@ubjs/react-native";
console.log(`UBRN_PLAYER_OK uniffi=${typeof (globalThis as any).uniffi}`);'
grep -q '"@ubjs/react-native"' package.json || { echo "player is not a direct dependency"; exit 1; }

echo "-- 4. build $PLATFORM"
if [ "$PLATFORM" = ios ]; then
  cd ios
  bundle install
  bundle exec pod install
  xcodebuild -workspace "$APP.xcworkspace" -scheme "$APP" -configuration Debug \
    -sdk iphonesimulator -destination 'generic/platform=iOS Simulator' \
    -derivedDataPath build CODE_SIGNING_ALLOWED=NO build | tail -40
  # A path pod builds in place, so the products are the proof: the shim
  # compiled into the pod's static library, and the Rust half embedded in the
  # app as a framework, which is what CocoaPods signs on device.
  PRODUCTS=build/Build/Products/Debug-iphonesimulator
  ls "$PRODUCTS/UbjsReactNative/libUbjsReactNative.a" >/dev/null
  ls "$PRODUCTS/$APP.app/Frameworks/UniffiRuntimeJsi.framework/UniffiRuntimeJsi" >/dev/null
else
  cd android
  ./gradlew assembleDebug --no-daemon | tail -40
  # The shim's .so was built for the shipped ABIs, and the Rust half was
  # packaged beside it.
  find app/build -name 'libubrn_jsi_player.so' | grep -E 'arm64-v8a|x86_64'
  find app/build -name 'libuniffi_runtime_jsi.so' | grep -E 'arm64-v8a|x86_64'
  # bionic reads a NEEDED entry containing '/' as a path relative to the
  # process CWD and never searches the app's native library directory, so the
  # shim has to name the Rust half by its bare soname.
  : "${ANDROID_NDK_HOME:?ANDROID_NDK_HOME must be set to inspect the built .so}"
  case "$(uname -s)" in
    Darwin) NDK_HOST=darwin-x86_64 ;;
    Linux)  NDK_HOST=linux-x86_64 ;;
    *) echo "unsupported host for the NDK toolchain: $(uname -s)"; exit 1 ;;
  esac
  READELF="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/$NDK_HOST/bin/llvm-readelf"
  # sed rather than head: pipefail turns head's early exit into a SIGPIPE failure.
  SHIM=$(find app/build -name 'libubrn_jsi_player.so' -path '*arm64-v8a*' | sed -n 1p)
  [ -n "$SHIM" ] || { echo "no arm64-v8a libubrn_jsi_player.so under app/build"; exit 1; }
  NEEDED=$("$READELF" -d "$SHIM" | sed -n 's/.*Shared library: \(\[.*uniffi_runtime_jsi.*\]\).*/\1/p')
  [ "$NEEDED" = "[libuniffi_runtime_jsi.so]" ] ||
    { echo "shim NEEDED ${NEEDED:-nothing} for the Rust half, want [libuniffi_runtime_jsi.so]"; exit 1; }
  echo "$SHIM NEEDED $NEEDED"
fi

if [ "$RUN" = true ]; then
  cd "$WORK/$APP"
  echo "-- 5. run $PLATFORM"
  if [ "$PLATFORM" = ios ]; then
    rnapp_run_ios "$WORK/$APP" "$APP" "UBRN_PLAYER_OK uniffi=object"
  else
    rnapp_run_android "$WORK/$APP" "$APP" "UBRN_PLAYER_OK uniffi=object"
  fi
  echo "✅ $PLATFORM: globalThis.uniffi is installed in a running app"
fi

echo "✅ $PLATFORM: @ubjs/react-native compiles and links in a fresh RN $RN_VERSION app"
if [ "$KEEP" = false ]; then rm -rf "$WORK"; fi
