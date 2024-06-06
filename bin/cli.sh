#!/usr/bin/env bash
#
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/
#
set -e
root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest_path="${root_dir}/crates/uniffi-bindgen-react-native/Cargo.toml"

executable="$root_dir/target/debug/uniffi-bindgen-react-native"
if [ ! -x "$executable" ]; then
    cargo build --manifest-path "${manifest_path}"
fi
 "$executable" "$@"
