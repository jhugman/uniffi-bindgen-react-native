#!/usr/bin/env bash
# The library lane: scaffold a React Native library around examples/arithmetic
# with create-react-native-library, build and generate it with `ubrn jsi2`,
# pack it, install it and the player into a fresh app, and run the app on the
# iOS simulator or an Android emulator until it logs a call into Rust.
set -euo pipefail

usage() {
  echo "Usage: $0 (--ios | --android) [--rn-version VERSION] [--work-dir DIR] [--keep]"
}

PLATFORM=""; RN_VERSION="latest"; WORK="/tmp/ubrn-jsi2-library"; KEEP=false
while [ $# -gt 0 ]; do
  case "$1" in
    --ios) PLATFORM=ios ;;
    --android) PLATFORM=android ;;
    --rn-version) RN_VERSION="$2"; shift ;;
    --work-dir) WORK="$2"; shift ;;
    --keep) KEEP=true ;;
    -h|--help) usage; exit 0 ;;
    *) usage; exit 1 ;;
  esac
  shift
done
[ -n "$PLATFORM" ] || { usage; exit 1; }
case "$WORK" in /|"") echo "refusing to use '$WORK' as the work dir"; exit 1 ;; esac

ROOT=$(git rev-parse --show-toplevel)
# shellcheck source=scripts/lib/rn-app.sh
source "$ROOT/scripts/lib/rn-app.sh"
trap rnapp_cleanup EXIT

LIB=arith-lib
APP=ArithSmoke
SENTINEL="UBRN_JSI2_OK add(2,3)=5"

echo "-- 1. ubrn, and the player's prebuilt Rust half for $PLATFORM"
cargo build -p uniffi-bindgen-react-native
UBRN="$ROOT/target/debug/uniffi-bindgen-react-native"
if [ "$PLATFORM" = ios ]; then
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

echo "-- 3. scaffold the library around examples/arithmetic"
rm -rf "$WORK"; mkdir -p "$WORK"
cd "$WORK"
# create-react-native-library has no `--example none`; scaffolding the vanilla
# example and deleting it is the only non-interactive way to a bare library.
npm_config_yes=true npx create-react-native-library@latest "$LIB" \
  --react-native-version "$RN_VERSION" --slug "$LIB" --description "An automated test" \
  --author-name "James" --author-email "noop@nomail.com" --author-url "https://nowhere.com/james" \
  --repo-url "https://github.com/jhugman/$LIB" --languages kotlin-objc --type turbo-module \
  --example vanilla --local false --directory "$LIB"
cd "$WORK/$LIB"
rm -rf example
# The scaffold's native module, C++ and codegen spec are what the player replaces.
rm -rf android/src/main/java ios cpp src
# ubrn needs a repository; bob's prepare step needs devDependencies we never
# install; Metro reads the react-native field straight from source.
jq --arg lib "$LIB" '
  .repository = { type: "git", url: ("git+https://github.com/jhugman/" + $lib) }
  | .main = "./src/index.tsx"
  | .["react-native"] = "./src/index.tsx"
  | del(.scripts.prepare) | del(.codegenConfig) | del(.devDependencies) | del(.workspaces)
' package.json > package.json.new && mv package.json.new package.json
cat > ubrn.config.yaml <<EOF
rust:
  directory: $ROOT/examples/arithmetic
  manifestPath: Cargo.toml
EOF

echo "-- 4. ubrn build jsi2 $PLATFORM --and-generate"
if [ "$PLATFORM" = ios ]; then
  "$UBRN" build jsi2 ios --config ubrn.config.yaml --and-generate --sim-only
  ls ios/arithmetical.xcframework/Info.plist
else
  # The emulator's own ABI, so one build serves the run.
  case "$(uname -m)" in arm64|aarch64) ABI=arm64-v8a ;; *) ABI=x86_64 ;; esac
  "$UBRN" build jsi2 android --config ubrn.config.yaml --and-generate --targets "$ABI"
  ls "android/src/main/jniLibs/$ABI/libarithmetical.so"
fi
grep -q 'import "@ubjs/react-native"' src/index.tsx
LIB_TGZ=$(npm pack --silent | tail -1)

echo "-- 5. a fresh app with the player and the library as direct dependencies"
rnapp_scaffold "$WORK" "$APP" "$RN_VERSION"
rnapp_install_tarballs "$WORK/$APP" "$ROOT/typescript/$CORE_TGZ" "$ROOT/runtimes/jsi/$PLAYER_TGZ" "$WORK/$LIB/$LIB_TGZ"
grep -q '"@ubjs/react-native"' "$WORK/$APP/package.json" || { echo "player is not a direct dependency"; exit 1; }
rnapp_prepend_app_tsx "$WORK/$APP" "import { add } from '$LIB';
console.log(\`UBRN_JSI2_OK add(2,3)=\${add(2n, 3n)}\`);"

echo "-- 6. run $PLATFORM"
if [ "$PLATFORM" = ios ]; then
  rnapp_run_ios "$WORK/$APP" "$APP" "$SENTINEL"
else
  rnapp_run_android "$WORK/$APP" "$APP" "$SENTINEL"
fi

echo "✅ $PLATFORM: a generated library called into Rust through @ubjs/react-native in a running RN $RN_VERSION app"
if [ "$KEEP" = false ]; then rm -rf "$WORK"; fi
