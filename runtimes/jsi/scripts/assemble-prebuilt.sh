#!/usr/bin/env bash
# Lays artifacts/<target>/libuniffi_runtime_jsi.{dylib,so} out as prebuilt/:
# android/<abi>/libuniffi_runtime_jsi.so for jniLibs, and one
# UniffiRuntimeJsi.framework bundle per iOS platform inside a dynamic
# xcframework, which CocoaPods embeds and signs. The xcframework step needs macOS.
set -euo pipefail

HERE=$(cd "$(dirname "$0")/.." && pwd)
IN="${1:-$HERE/artifacts}"
OUT="$HERE/prebuilt"
NAME=UniffiRuntimeJsi
MIN_IOS_VERSION=15.1
VERSION=$(node -p 'require(process.argv[1]).version' "$HERE/package.json")

abi_for() {
  case "$1" in
    aarch64-linux-android) echo arm64-v8a ;;
    x86_64-linux-android)  echo x86_64 ;;
    armv7-linux-androideabi) echo armeabi-v7a ;;
    i686-linux-android)    echo x86 ;;
    *) return 1 ;;
  esac
}

# make_framework DIR DYLIB PLATFORM: DIR/UniffiRuntimeJsi.framework/{UniffiRuntimeJsi,Info.plist},
# where PLATFORM is iPhoneOS or iPhoneSimulator: Xcode-built frameworks carry
# CFBundleSupportedPlatforms and App Store validation expects it of an embedded
# framework. The install name is the path the app's loader looks up once
# CocoaPods has embedded the bundle under Frameworks/.
make_framework() {
  local dir="$1" dylib="$2" platform="$3"
  local fw="$dir/$NAME.framework"
  mkdir -p "$fw"
  cp "$dylib" "$fw/$NAME"
  install_name_tool -id "@rpath/$NAME.framework/$NAME" "$fw/$NAME"
  cat > "$fw/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key><string>en</string>
	<key>CFBundleExecutable</key><string>$NAME</string>
	<key>CFBundleIdentifier</key><string>dev.ubjs.reactnative.$NAME</string>
	<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
	<key>CFBundleName</key><string>$NAME</string>
	<key>CFBundlePackageType</key><string>FMWK</string>
	<key>CFBundleShortVersionString</key><string>$VERSION</string>
	<key>CFBundleSupportedPlatforms</key><array><string>$platform</string></array>
	<key>CFBundleVersion</key><string>1</string>
	<key>MinimumOSVersion</key><string>$MIN_IOS_VERSION</string>
</dict>
</plist>
EOF
  echo "$fw"
}

rm -rf "$OUT"; mkdir -p "$OUT"
device=""; sims=()
for dir in "$IN"/*/; do
  t=$(basename "$dir")
  case "$t" in
    *-linux-android*)
      lib="$dir/libuniffi_runtime_jsi.so"
      [ -f "$lib" ] || continue
      mkdir -p "$OUT/android/$(abi_for "$t")"
      cp "$lib" "$OUT/android/$(abi_for "$t")/" ;;
    aarch64-apple-ios)
      lib="$dir/libuniffi_runtime_jsi.dylib"
      [ -f "$lib" ] || continue
      device="$lib" ;;
    aarch64-apple-ios-sim|x86_64-apple-ios)
      lib="$dir/libuniffi_runtime_jsi.dylib"
      [ -f "$lib" ] || continue
      sims+=("$lib") ;;
  esac
done

if [ -n "$device" ] || [ ${#sims[@]} -gt 0 ]; then
  tmp=$(mktemp -d)
  trap 'rm -rf "$tmp"' EXIT
  args=()
  if [ -n "$device" ]; then
    mkdir -p "$tmp/ios"
    args+=(-framework "$(make_framework "$tmp/ios" "$device" iPhoneOS)")
  fi
  if [ ${#sims[@]} -gt 0 ]; then
    # One simulator slice may hold both arm64 and x86_64; lipo them together.
    mkdir -p "$tmp/ios-simulator"
    lipo -create "${sims[@]}" -output "$tmp/ios-simulator/libuniffi_runtime_jsi.dylib"
    args+=(-framework "$(make_framework "$tmp/ios-simulator" "$tmp/ios-simulator/libuniffi_runtime_jsi.dylib" iPhoneSimulator)")
  fi
  mkdir -p "$OUT/ios"
  xcodebuild -create-xcframework "${args[@]}" -output "$OUT/ios/$NAME.xcframework"
fi
echo "-- prebuilt:"; find "$OUT" -type f | sort
