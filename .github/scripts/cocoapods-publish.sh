#!/usr/bin/env bash
# Pushes the podspec to CocoaPods trunk, judging the outcome by trunk's own list
# of versions. Trunk records a version before it commits to CocoaPods/Specs, and
# that commit often reports a timeout or a 500 when the version has landed.
set -euo pipefail

POD=uniffi-bindgen-react-native
VERSION=$(node -p "require('./package.json').version")

on_trunk() {
  curl -fsS "https://trunk.cocoapods.org/api/v1/pods/$POD" |
    jq -e --arg v "$VERSION" 'any(.versions[]; .name == $v)' > /dev/null
}

# A re-run would otherwise fail as a duplicate push.
if on_trunk; then
  echo "$POD $VERSION is already on trunk"
  exit 0
fi

if pod trunk push "$POD.podspec"; then
  exit 0
fi

echo "::warning::pod trunk push reported a failure; checking trunk for $POD $VERSION"
for _ in $(seq 10); do
  if on_trunk; then
    echo "$POD $VERSION is on trunk"
    exit 0
  fi
  sleep 30
done

echo "::error::$POD $VERSION is not on trunk"
exit 1
