#!/usr/bin/env bash
set -e
root=.

for test in "${root}"/typescript/tests/*.test.ts ; do
    echo "Running test $test"
    cargo xtask run "${test}"
    echo
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
    fixtures=${selected_fixtures[*]}
fi

for fixture in ${fixtures} ; do
    if [[ " ${excluded_fixtures[@]} " =~ " ${fixture} " ]]; then
        continue
    fi
    echo "Running fixture ${fixture}"
    # This should all go in either an xtask or into our uniffi-bindgen command.
    # This builds the crate into the target dir.
    fixture_dir="${root}/fixtures/${fixture}"
    test_file="${fixture_dir}/tests/bindings/test_${fixture//-/_}.ts"

    out_dir="${fixture_dir}/generated"
    rm -Rf "${out_dir}" 2>/dev/null

    cpp_dir="${out_dir}/cpp"
    ts_dir="${out_dir}"
    # This command discovers where the lib is, generates the ts, cpp and hpp files,
    # and builds the generated C++ against it and hermes.
    # Optionally, it could build the crate for us.
    # Generate hermes flavoured JS from typescript, and runs the test.
    cargo xtask run \
        --no-cargo \
        --cpp-dir "${cpp_dir}" \
        --ts-dir "${ts_dir}" \
        --crate "${fixture_dir}" \
        "${test_file}"
    echo
done
