#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [[ "$(uname -s)" != "Darwin" ]]; then
  printf 'MVP release smoke must run on macOS; current OS is %s.\n' "$(uname -s)" >&2
  exit 1
fi

run_rust() {
  if command -v mise >/dev/null 2>&1; then
    mise exec rust@1.96.0 -- "$@"
  else
    "$@"
  fi
}

run_rust rustup target add aarch64-apple-darwin x86_64-apple-darwin

npm run check:all
npm run smoke:mvp
npm run tauri:build:macos
npm run package:verify

printf '%s\n' "MVP release smoke passed"
