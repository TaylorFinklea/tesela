#!/usr/bin/env bash

set -euo pipefail
cd "$(dirname "$0")/.."

SOURCE="release-notes/releases.json"
IOS_COPY="app/Tesela-iOS/Sources/Data/ReleaseNotes.json"

node scripts/changelog.mjs validate

if ! diff -u "$SOURCE" "$IOS_COPY"; then
  echo "" >&2
  echo "RELEASE NOTES DRIFT — $IOS_COPY does not match $SOURCE" >&2
  echo "Regenerate: cp $SOURCE $IOS_COPY" >&2
  exit 1
fi

echo "release notes in sync ($SOURCE == $IOS_COPY)"
