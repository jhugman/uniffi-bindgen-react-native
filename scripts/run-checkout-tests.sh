#!/usr/bin/env bash

set -e

ubrn=./bin/cli

function clean_up {
  rm -rf rust_modules
}

function announce {
  echo -e "\033[0;36m[TEST] $1\033[0m"
}

rc=0

function assert_eq {
  if [[ $1 == $2 ]]; then
    echo "✅ OK: '$1' == '$2'"
  else
    echo "❌ FAILURE: '$1' == '$2'"
    rc=1
  fi
}

function assert_contains {
  if [[ $1 == *$2* ]]; then
    echo "✅ OK: contains '$2'"
  else
    echo "❌ FAILURE: contains '$2'"
    rc=1
  fi
}

function assert_does_not_contain {
  if [[ $1 != *$2* ]]; then
    echo "✅ OK: does not contain '$2'"
  else
    echo "❌ FAILURE: does not contain '$2'"
    rc=1
  fi
}

clean_up

announce "checkout with default"
stderr=$("$ubrn" checkout https://github.com/actions/checkout 2> >(tee /dev/stderr))
pushd rust_modules/checkout
assert_eq $(git ls-remote --heads origin | grep refs/heads/main | cut -f1) $(git rev-parse HEAD)
assert_does_not_contain "$stderr" "detached HEAD"
popd

clean_up

announce "checkout with branch"
stderr=$("$ubrn" checkout https://github.com/actions/checkout --branch releases/v1 2> >(tee /dev/stderr))
pushd rust_modules/checkout
assert_eq $(git ls-remote --heads origin | grep refs/heads/releases/v1 | cut -f1) $(git rev-parse HEAD)
assert_does_not_contain "$stderr" "detached HEAD"
popd

clean_up

announce "checkout with tag"
stderr=$("$ubrn" checkout https://github.com/actions/checkout --branch v4.0.0 2> >(tee /dev/stderr))
pushd rust_modules/checkout
assert_eq $(git ls-remote --tags origin | grep refs/tags/v4.0.0 | cut -f1) $(git rev-parse HEAD)
assert_contains "$stderr" "detached HEAD"
popd

clean_up

announce "checkout with sha"
stderr=$("$ubrn" checkout https://github.com/actions/checkout --branch c533a0a4cfc4962971818edcfac47a2899e69799 2> >(tee /dev/stderr))
pushd rust_modules/checkout
assert_eq c533a0a4cfc4962971818edcfac47a2899e69799 $(git rev-parse HEAD)
assert_contains "$stderr" "detached HEAD"
popd

clean_up

announce "update checkout from sha to sha"
"$ubrn" checkout https://github.com/actions/checkout --branch c533a0a4cfc4962971818edcfac47a2899e69799
"$ubrn" checkout https://github.com/actions/checkout --branch 2d7d9f7ff5b310f983d059b68785b3c74d8b8edd
pushd rust_modules/checkout
assert_eq 2d7d9f7ff5b310f983d059b68785b3c74d8b8edd $(git rev-parse HEAD)
popd

clean_up

announce "update checkout from tag to sha"
"$ubrn" checkout https://github.com/actions/checkout --branch v4.0.0
"$ubrn" checkout https://github.com/actions/checkout --branch 2d7d9f7ff5b310f983d059b68785b3c74d8b8edd
pushd rust_modules/checkout
assert_eq 2d7d9f7ff5b310f983d059b68785b3c74d8b8edd $(git rev-parse HEAD)
popd

clean_up

exit $rc
