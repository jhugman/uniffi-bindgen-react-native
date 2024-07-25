#!/usr/bin/env bash
#
# This Source Code Form is subject to the terms of the Mozilla Public
# License, v. 2.0. If a copy of the MPL was not distributed with this
# file, You can obtain one at http://mozilla.org/MPL/2.0/
#
set -e
script_dir="$(dirname "${BASH_SOURCE[0]}")"
if [[ "$script_dir" == *".bin" ]] ; then
    root_dir="$(cd "$script_dir/../uniffi-bindgen-react-native" && pwd)"
elif [[ "$script_dir" == *"bin" ]] ; then
    root_dir="$(cd "$script_dir/.." && pwd)"
else
    echo "Unable to locate the uniffi-bindgen-react-native directory" 2>/dev/null
    exit 1
fi

manifest_path="${root_dir}/crates/ubrn_cli/Cargo.toml"

cargo run --quiet --manifest-path "${manifest_path}" -- "$@"
