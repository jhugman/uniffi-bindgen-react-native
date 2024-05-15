#!/usr/bin/env bash
root=.

for test in "${root}"/typescript/tests/*.test.ts ; do
    echo "Running $test"
    cargo xtask run "${test}"
done

uniffi_bindgen_manifest="${root}/crates/uniffi-bindgen-react-native/Cargo.toml"
for fixture in $(cd "${root}/fixtures" && ls) ; do
    # This should all go in either an xtask or into our uniffi-bindgen command.
    # This builds the crate into the target dir.
    fixture_dir="${root}/fixtures/${fixture}"
    cargo build --manifest-path "${fixture_dir}/Cargo.toml"

    # Generate the ts, cpp and hpp files into "${fixture_dir}/generated"
    # We should use the so or dylib file here but for now we can just use the UDL
    # fie.
    cargo run --manifest-path "$uniffi_bindgen_manifest" -- \
        "${fixture_dir}/src/${fixture}.udl" \
        --out-dir "${fixture_dir}/generated"
    # This command discovers where the lib is, and builds the generated C++
    # against it and hermes. Optionally, it could build the crate for us.
    # Generate hermes flavoured JS from typescript, and runs the test.
    cargo xtask run "${fixture_dir}/tests/bindings/test_${fixture}.ts" \
        --cpp "${fixture_dir}/generated/${fixture}.cpp" \
        --crate "${fixture_dir}" \
        --no-cargo
done
