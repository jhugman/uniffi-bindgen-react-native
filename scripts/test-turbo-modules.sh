#!/bin/bash
set -e

ROOT=
PROJECT_DIR=my-test-library
KEEP_ROOT_ON_ERROR=false
BOB_VERSION=latest
PROJECT_SLUG=my-test-library
FORCE_NEW_DIR=false
IOS_NAME=MyTestLibrary
SKIP_IOS=false
SKIP_ANDROID=false
UBRN_CONFIG=
UBRN_BIN=
PWD=

usage() {
  echo "Usage: $0 [options] [PROJECT_DIR]"
  echo ""
  echo "Options:"
  echo "  -A, --skip-android                 Skip building for Android."
  echo "  -I, --skip-ios                     Skip building for iOS."
  echo "  -u, --builder-bob-version VERSION  Specify the version of builder-bob to use."
  echo "  -s, --slug PROJECT_SLUG            Specify the project slug."
  echo "  -i, --ios-name IOS_NAME            Specify the iOS project name."
  echo "  -k, --keep-directory-on-exit       Keep the PROJECT_DIR directory even if an error occurs."
  echo "  -f, --force-new-directory          If PROJECT_DIR directory exist, remove it first."
  echo "  -h, --help                         Display this help message."
  echo ""
  echo "Arguments:"
  echo "  PROJECT_DIR                               Specify the root directory for the project (default: my-test-library)."
}

cleanup() {
  if [ "$KEEP_ROOT_ON_ERROR" == false ] && [ -d "$PROJECT_DIR" ]; then
    echo "Removing $PROJECT_DIR..."
    rm -rf "$PROJECT_DIR"
  fi
  cd "$PWD"
}

error() {
  echo "Error: $1"
  cleanup
  exit 1
}

find_git_project_root() {
  git rev-parse --show-toplevel 2>/dev/null || {
    echo "Project root not found" >&2
    return 1
  }
}

derive_paths() {
  ROOT=$(find_git_project_root)
  UBRN_BIN="$ROOT/bin/cli"
  PWD=$(pwd)
}

parse_cli_options() {
  # Parse command line options
  while [ $# -gt 0 ]; do
    case "$1" in
      -u|--builder-bob-version)
        BOB_VERSION="$2"
        shift
        ;;
      -s|--slug)
        PROJECT_SLUG="$2"
        shift
        ;;
      -i|--ios-name)
        IOS_NAME="$2"
        shift
        ;;
      -C|--ubrn-config)
        local config_file
        config_file="$2"
        if [[ "$config_file" = /* ]] ; then
          UBRN_CONFIG="$config_file"
        else
          UBRN_CONFIG="$PWD/$config_file"
        fi
        shift
        ;;
      -k|--keep-directory-on-exit)
        KEEP_ROOT_ON_ERROR=true
        ;;
      -f|--force-new-directory)
        FORCE_NEW_DIR=true
        ;;
      -A|--skip-android)
        SKIP_ANDROID=true
        ;;
      -I|--skip-ios)
        SKIP_IOS=true
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      -*)
        KEEP_ROOT_ON_ERROR=true
        error "Bad argument: $1"
        ;;
      *)
        PROJECT_DIR="$1"
        ;;
    esac
    shift
  done
  # Ensure PROJECT_DIR is specified
  if [ -z "$PROJECT_DIR" ]; then
    PROJECT_DIR=my-test-library
  fi

  echo "-- PROJECT_DIR = $PROJECT_DIR"
  echo "-- PROJECT_SLUG = $PROJECT_SLUG"
  echo "-- IOS_NAME = $IOS_NAME"

}



create_library() {
  local directory
  local base
  directory=$(dirname "$PROJECT_DIR")
  base=$(basename "$PROJECT_DIR")
  if [ ! -d "$directory" ]; then
    mkdir -p "$directory" || error "Cannot create $directory"
  fi

  pushd "$directory" || error "Cannot enter $directory"

  if [ "$FORCE_NEW_DIR" == true ] && [ -d "$base" ]; then
    rm -rf "$base" || error "Failed to remove existing directory $base"
  fi

  local example_type
  if [ "$BOB_VERSION" == "latest" ] ; then
    example_type=test-app
  fi
  npx create-react-native-library@$BOB_VERSION \
    --slug "$PROJECT_SLUG" \
    --description "An automated test" \
    --author-name "James" \
    --author-email "noop@nomail.com" \
    --author-url "https://nowhere.com/james" \
    --repo-url "https://github.com/jhugman/uniffi-bindgen-react-native" \
    --languages cpp \
    --type module-new \
    --example $example_type \
    "$base" || error "Failed to create library in $PROJECT_DIR"
  popd || error "Cannot exit $directory"
}

install_dependencies() {
  pushd "$PROJECT_DIR" || error "Failed to navigate to $PROJECT_DIR"
  # touch yarn.lock
  yarn || error "Failed to install dependencies"
  # rm yarn.lock
  popd || error "Failed to return from $PROJECT_DIR"
}

install_example_dependencies() {
  pushd "$PROJECT_DIR/example" || error "Failed to navigate to $PROJECT_DIR/example"
  # touch yarn.lock
  yarn || error "Failed to install example dependencies"
  # rm yarn.lock
  # rm -Rf .yarn
  popd || error "Failed to return from $PROJECT_DIR/example"
}

check_deleted_files() {
  local extensions="$1"
  local deleted_files
  deleted_files=$(git status --porcelain | grep '^ D' || true | grep -E "\\.(${extensions// /|})$" || true )

  echo "-- finished grep"
  if [ -n "$deleted_files" ]; then
    echo "Error: The following files have been deleted:"
    echo "$deleted_files"
    error
  fi
}

generate_turbo_module() {
  pushd "$PROJECT_DIR" || error "Can't enter $PROJECT_DIR"
  echo "-- Running $UBRN_BIN in PROJECT_DIR = $(pwd)"
  rm -Rf cpp/ android/src/main/java ios/ src/Native* src/generated/ src/index.ts*
  "$UBRN_BIN" checkout --config "$UBRN_CONFIG"
  "$UBRN_BIN" build ios --config "$UBRN_CONFIG" --and-generate --targets aarch64-apple-ios-sim

  local jvm_lang
  if [ "$BOB_VERSION" == "latest" ] ; then
    jvm_lang=kt
  else
    jvm_lang=java
  fi
  echo "-- Checking for deleted files"
  check_deleted_files "$jvm_lang h mm ts podspec tsx"
  echo "-- No deleted files detected"

  popd || error "Can't exit $PROJECT_DIR"
}

copy_into_node_modules() {
  # Source and destination directories
  local SRC_DIR="$ROOT"
  local DEST_DIR="$PROJECT_DIR/node_modules/uniffi-bindgen-react-native"

  # Use rsync to copy contents, excluding cpp_modules and rust_modules directories
  rsync -av \
    --exclude 'cpp_modules' \
    --exclude 'rust_modules' \
    --exclude 'build' \
    --exclude 'target' \
  "$SRC_DIR/" "$DEST_DIR/"
}

build_android_example() {
  pushd "$PROJECT_DIR" || error "Failed to navigate to $PROJECT_DIR"
  "$UBRN_BIN" build android --config "$UBRN_CONFIG"
  popd || error "Failed to exit $PROJECT_DIR"

  pushd "$PROJECT_DIR/example/android" || error "Failed to navigate to $PROJECT_DIR/example/android"
  ./gradlew build || error "Failed to build Android example"
  popd || error "Failed to return from $PROJECT_DIR/example/android"
}

build_ios_example() {
  pushd "$PROJECT_DIR/example/ios" || error "Failed to navigate to $PROJECT_DIR/example/ios"
  echo "pod 'uniffi-bindgen-react-native', :path => '../../node_modules/uniffi-bindgen-react-native'" >> Podfile
  pod install || error "Cannot run Podfile"

  # Find the UDID of the first booted device, or fall back to the first available device
  udid=$(xcrun simctl list --json devices | jq -r '.devices | to_entries | .[].value | map(select(.state == "Booted")) | .[0].udid')
  if [ "$udid" == "null" ]; then
    udid=$(xcrun simctl list --json devices | jq -r '.devices | to_entries | .[].value | map(select(.isAvailable == true)) | .[0].udid')
  fi

  if [ "$udid" == "null" ]; then
    error "No available iOS simulator found"
  fi

  xcodebuild -workspace "${IOS_NAME}Example.xcworkspace" -scheme "${IOS_NAME}Example" -configuration Debug -destination "id=$udid" || error "Failed to build iOS example"
  popd || error "Failed to return from $PROJECT_DIR/example/ios"
}

main() {
  create_library
  install_dependencies
  if [ -n "$UBRN_CONFIG" ]; then
    generate_turbo_module
  fi
  install_example_dependencies
  copy_into_node_modules
  if [ "$SKIP_ANDROID" == false ]; then
    build_android_example
  fi
  if [ "$SKIP_IOS" == false ]; then
    build_ios_example
  fi
  cleanup
}
derive_paths
parse_cli_options "$@"
main
