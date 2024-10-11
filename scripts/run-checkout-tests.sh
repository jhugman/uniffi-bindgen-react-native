#!/usr/bin/env bash

set -e

ubrn=./bin/cli

function clean_up {
  rm -rf rust_modules
}

function announce {
  echo "[TEST] $1"
}

rc=0

function assert_eq {
  if [[ $1 == $2 ]]; then
    echo "✅ OK"
  else
    echo "❌ FAILURE: '$1' != '$2'"
    rc=1
  fi
}

clean_up

announce "checkout with default"
"$ubrn" checkout https://github.com/actions/checkout
pushd rust_modules/checkout
assert_eq $(git ls-remote --heads origin | grep refs/heads/main | cut -f1) $(git rev-parse HEAD)
popd

clean_up

announce "checkout with branch"
"$ubrn" checkout https://github.com/actions/checkout --branch releases/v1
pushd rust_modules/checkout
assert_eq $(git ls-remote --heads origin | grep refs/heads/releases/v1 | cut -f1) $(git rev-parse HEAD)
popd

clean_up

announce "checkout with tag"
"$ubrn" checkout https://github.com/actions/checkout --branch v4.0.0
pushd rust_modules/checkout
assert_eq $(git ls-remote --tags origin | grep refs/tags/v4.0.0 | cut -f1) $(git rev-parse HEAD)
popd

clean_up

announce "checkout with sha"
"$ubrn" checkout https://github.com/actions/checkout --branch c533a0a4cfc4962971818edcfac47a2899e69799
pushd rust_modules/checkout
assert_eq c533a0a4cfc4962971818edcfac47a2899e69799 $(git rev-parse HEAD)
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
