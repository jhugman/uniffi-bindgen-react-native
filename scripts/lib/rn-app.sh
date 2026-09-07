#!/usr/bin/env bash
# Shared by the player and library runtime lanes: scaffold a plain React
# Native app, install packed tarballs into it, then launch it on a simulator
# or emulator with Metro serving and wait for a console sentinel. Debug builds
# throughout: RCTLog only forwards console.log in a debug build, so a sentinel
# from a release build would never be logged at all.
#
# Callers `source` this file after `set -euo pipefail` and call rnapp_cleanup
# from their EXIT trap.

RNAPP_METRO_PID=""
RNAPP_EMULATOR_PID=""
RNAPP_SIM_UDID=""

rnapp_log() { echo "-- $*"; }

# rnapp_scaffold DIR APP RN_VERSION: a fresh app at DIR/APP, dependencies installed.
rnapp_scaffold() {
  local dir=$1 app=$2 rn_version=$3
  rm -rf "$dir"; mkdir -p "$dir"
  npx --yes @react-native-community/cli@latest init "$app" \
    --version "$rn_version" --directory "$dir/$app" --skip-install --skip-git-init --pm npm
  (cd "$dir/$app" && npm install)
}

# rnapp_install_tarballs APP_DIR TGZ...: direct dependencies, which is the only
# kind autolinking reads.
rnapp_install_tarballs() {
  local app_dir=$1; shift
  (cd "$app_dir" && npm install "$@")
}

# rnapp_prepend_app_tsx APP_DIR SNIPPET: SNIPPET runs when the bundle evaluates
# App.tsx, before any UI.
rnapp_prepend_app_tsx() {
  local app_dir=$1 snippet=$2
  printf '%s\n%s' "$snippet" "$(cat "$app_dir/App.tsx")" > "$app_dir/App.tsx"
}

rnapp_start_metro() {
  local app_dir=$1
  (cd "$app_dir" && npx react-native start --port 8081 > "$app_dir/metro.log" 2>&1) &
  RNAPP_METRO_PID=$!
  rnapp_log "metro pid $RNAPP_METRO_PID, log $app_dir/metro.log"
}

# rnapp_wait_for FILE SENTINEL SECONDS: succeed when SENTINEL appears in FILE.
rnapp_wait_for() {
  local file=$1 sentinel=$2 secs=$3 i=0
  while [ "$i" -lt "$secs" ]; do
    if grep -q -- "$sentinel" "$file" 2>/dev/null; then
      rnapp_log "sentinel seen after ${i}s: $(grep -m1 -- "$sentinel" "$file")"
      return 0
    fi
    sleep 1; i=$((i + 1))
  done
  echo "❌ sentinel '$sentinel' not seen in $file after ${secs}s; tail:" >&2
  tail -60 "$file" >&2
  return 1
}

# The first available iPhone simulator with an iOS runtime, booted.
rnapp_boot_simulator() {
  RNAPP_SIM_UDID=$(xcrun simctl list devices available -j | python3 -c '
import json, sys
d = json.load(sys.stdin)["devices"]
for runtime, devices in d.items():
    if "iOS" not in runtime: continue
    for dev in devices:
        if dev["name"].startswith("iPhone"):
            print(dev["udid"]); sys.exit(0)
sys.exit("no available iPhone simulator")')
  xcrun simctl boot "$RNAPP_SIM_UDID" 2>/dev/null || true
  xcrun simctl bootstatus "$RNAPP_SIM_UDID" -b
  rnapp_log "simulator $RNAPP_SIM_UDID booted"
}

# rnapp_run_ios APP_DIR APP SENTINEL: pod install, build for the booted
# simulator, install, launch with the console captured, wait for SENTINEL.
rnapp_run_ios() {
  local app_dir=$1 app=$2 sentinel=$3
  rnapp_boot_simulator
  (cd "$app_dir/ios" && bundle install && bundle exec pod install)
  (cd "$app_dir/ios" && xcodebuild -workspace "$app.xcworkspace" -scheme "$app" -configuration Debug \
    -sdk iphonesimulator -destination "platform=iOS Simulator,id=$RNAPP_SIM_UDID" \
    -derivedDataPath build CODE_SIGNING_ALLOWED=NO build | tail -20)
  local bundle="$app_dir/ios/build/Build/Products/Debug-iphonesimulator/$app.app"
  local bundle_id
  bundle_id=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "$bundle/Info.plist")
  rnapp_start_metro "$app_dir"
  xcrun simctl install "$RNAPP_SIM_UDID" "$bundle"
  xcrun simctl terminate "$RNAPP_SIM_UDID" "$bundle_id" 2>/dev/null || true
  # RCTLog writes JS console output to os_log at info level, not to the stderr
  # `simctl launch --console-pty` streams; dropping the Apple subsystems leaves
  # the app's own lines readable when the sentinel never arrives.
  xcrun simctl spawn "$RNAPP_SIM_UDID" log stream --level info --style compact \
    --predicate "process == \"$app\" and NOT (subsystem beginswith \"com.apple\")" \
    > "$app_dir/app.log" 2>&1 &
  # The stream prints its filter banner once attached; launching before that
  # races the first console line.
  local i=0
  until grep -q 'Filtering the log data' "$app_dir/app.log" 2>/dev/null || [ "$i" -ge 30 ]; do
    sleep 1; i=$((i + 1))
  done
  xcrun simctl launch "$RNAPP_SIM_UDID" "$bundle_id"
  rnapp_wait_for "$app_dir/app.log" "$sentinel" 240
  xcrun simctl terminate "$RNAPP_SIM_UDID" "$bundle_id" 2>/dev/null || true
}

rnapp_boot_emulator() {
  local avd=${UBRN_AVD:?set UBRN_AVD to an AVD from 'emulator -list-avds'}
  local sdk=${ANDROID_HOME:-$HOME/Library/Android/sdk}
  if ! "$sdk/platform-tools/adb" get-state >/dev/null 2>&1; then
    "$sdk/emulator/emulator" -avd "$avd" -no-window -no-audio -no-boot-anim -no-snapshot > /tmp/ubrn-emulator.log 2>&1 &
    RNAPP_EMULATOR_PID=$!
    rnapp_log "emulator $avd pid $RNAPP_EMULATOR_PID"
  fi
  "$sdk/platform-tools/adb" wait-for-device
  until [ "$("$sdk/platform-tools/adb" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do sleep 2; done
  rnapp_log "emulator booted: $("$sdk/platform-tools/adb" shell getprop ro.product.cpu.abi | tr -d '\r')"
}

# rnapp_run_android APP_DIR APP SENTINEL: boot the AVD, installDebug, start the
# activity with Metro reachable through adb reverse, wait for SENTINEL in logcat.
rnapp_run_android() {
  local app_dir=$1 app=$2 sentinel=$3
  local sdk=${ANDROID_HOME:-$HOME/Library/Android/sdk}
  local adb="$sdk/platform-tools/adb"
  rnapp_boot_emulator
  rnapp_start_metro "$app_dir"
  "$adb" reverse tcp:8081 tcp:8081
  (cd "$app_dir/android" && ./gradlew installDebug --no-daemon | tail -20)
  local pkg
  pkg=$(echo "$app" | tr '[:upper:]' '[:lower:]')
  "$adb" logcat -c
  "$adb" shell am start -n "com.$pkg/.MainActivity"
  # ReactNativeJS is the tag console output lands under.
  "$adb" logcat -s ReactNativeJS:* > "$app_dir/app.log" 2>&1 &
  rnapp_wait_for "$app_dir/app.log" "$sentinel" 240
  "$adb" shell am force-stop "com.$pkg" || true
}

rnapp_cleanup() {
  # Metro's node server outlives the subshell RNAPP_METRO_PID names, and a
  # survivor holds port 8081 against the next run, so match it by command line.
  if [ -n "$RNAPP_METRO_PID" ]; then kill "$RNAPP_METRO_PID" 2>/dev/null || true; fi
  pkill -f 'react-native start --port 8081' 2>/dev/null || true
  pkill -f 'adb logcat -s ReactNativeJS' 2>/dev/null || true
  pkill -f 'simctl spawn .* log stream' 2>/dev/null || true
  if [ -n "$RNAPP_EMULATOR_PID" ]; then kill "$RNAPP_EMULATOR_PID" 2>/dev/null || true; fi
}
