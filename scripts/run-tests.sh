#!/usr/bin/env bash
set -e
root=.

for test in "${root}"/typescript/tests/*.test.ts ; do
    echo "Running $test"
    cargo xtask run "${test}"
done

declare -a selected_fixtures=()
declare -a excluded_fixtures=()
while (( "$#" )); do
  case "$1" in
    '--fixture'|'-f')
       selected_fixtures+=("$2")
       shift 2
       ;;
    '--exclude'|'-x')
       excluded_fixtures+=("$2")
       shift 2
       ;;
    *)
       echo "Unknown argument: $1"
       exit 1
       ;;
  esac
done

if [ ${#selected_fixtures[@]} -eq 0 ]; then
    fixtures=$(cd "${root}/fixtures" && ls)
else
    fixtures=${selected_fixtures[@]}
fi

uniffi_bindgen_manifest="${root}/crates/ubrn_cli/Cargo.toml"
for fixture in ${fixtures} ; do
    if [[ " ${excluded_fixtures[@]} " =~ " ${fixture} " ]]; then
        continue
    fi
    # This should all go in either an xtask or into our uniffi-bindgen command.
    # This builds the crate into the target dir.
    fixture_dir="${root}/fixtures/${fixture}"
    cargo build --manifest-path "${fixture_dir}/Cargo.toml"

    out_dir="${fixture_dir}/generated"
    rm -Rf "${out_dir}" 2>/dev/null

    cpp_dir="${out_dir}/cpp"
    ts_dir="${out_dir}"
    # Generate the ts, cpp and hpp files into "${fixture_dir}/generated"
    # We should use the so or dylib file here but for now we can just use the UDL
    # fie.
    RUST_BACKTRACE=1 cargo run --manifest-path "$uniffi_bindgen_manifest" -- \
        generate \
        bindings \
        --ts-dir "${ts_dir}" --cpp-dir "${cpp_dir}" \
        "${fixture_dir}/src/${fixture}.udl" \
    # This command discovers where the lib is, and builds the generated C++
    # against it and hermes. Optionally, it could build the crate for us.
    # Generate hermes flavoured JS from typescript, and runs the test.
    cargo xtask run "${fixture_dir}/tests/bindings/test_${fixture}.ts" \
        --cpp "${cpp_dir}/${fixture}.cpp" \
        --crate "${fixture_dir}" \
        --no-cargo
done
