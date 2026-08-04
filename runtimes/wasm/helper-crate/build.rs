/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
fn main() {
    // Required so the player can install trampolines into the cdylib's
    // exported function table at runtime.
    //
    // Gate on the build target: these are wasm-ld flags that the host linker
    // (cc/ld) does not understand, so emitting them on host targets breaks
    // host test builds (`cargo test -p uniffi-runtime-wasm`).
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.starts_with("wasm32") {
        println!("cargo:rustc-link-arg=--export-table");
        println!("cargo:rustc-link-arg=--growable-table");
    }
}
