#!/usr/bin/env bash

assert_desktop_web_bundle() {
  local app_bundle="${1:?app bundle path is required}"
  local web_index="$app_bundle/Contents/Resources/web/index.html"

  if [[ ! -f "$web_index" ]]; then
    echo "ERROR: desktop app is missing its bundled web UI at $web_index" >&2
    return 1
  fi
}
