#!/usr/bin/env bash
set -e
root=.

declare -a selected_fixtures=()
declare -a excluded_fixtures=()
flavor="jsi"
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
    '--flavor'|'-F')
       flavor="$2"
       shift 2
       ;;

    '--debug')
       set -x
       shift
       ;;
    *)
       echo "Unknown argument: $1"
       exit 1
       ;;
  esac
done

supports_flavor() {
  local fixture="$1"
  local flavor="$2"

  if [[ "$flavor" == "jsi" ]]; then
    return 0
  fi

  local flavor_file="${fixture}/tests/bindings/.supported-flavors.txt"
  if [[ -f "$flavor_file" ]]; then
    if grep -q "$flavor" "$flavor_file"; then
      return 0
    fi
  fi

  return 1
}

for test in "${root}"/typescript/tests/*.test.ts ; do
    echo "Running test $test"
    cargo xtask run "${test}" --flavor "$flavor"
    echo
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

    # This should all go in either an xtask or into our uniffi-bindgen command.
    # This builds the crate into the target dir.
    fixture_dir="${root}/fixtures/${fixture}"

    if ! supports_flavor "${fixture_dir}" "${flavor}"; then
      echo "Skipping fixture ${fixture}"
      continue
    fi

    test_file="${fixture_dir}/tests/bindings/test_${fixture//-/_}.ts"
    config_file="${fixture_dir}/uniffi.toml"
    out_dir="${fixture_dir}/generated"

    echo "Running fixture ${fixture}"
    rm -Rf "${out_dir}" 2>/dev/null

    cpp_dir="${out_dir}/${flavor}"
    ts_dir="${out_dir}"
    # This command discovers where the lib is, generates the ts, cpp and hpp files,
    # and builds the generated C++ against it and hermes.
    # Optionally, it could build the crate for us.
    # Generate hermes flavoured JS from typescript, and runs the test.
    cargo xtask run \
        --abi-dir "${cpp_dir}" \
        --ts-dir "${ts_dir}" \
        --toml "${config_file}" \
        --crate "${fixture_dir}" \
        --flavor "$flavor" \
        "${test_file}"
    echo

    # Clean up generated dir so that CI doesn't run out of disk
    rm -Rf "${out_dir}" 2>/dev/null
done
