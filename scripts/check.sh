#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

run_rust() {
  if command -v mise >/dev/null 2>&1; then
    mise exec rust@1.96.0 -- "$@"
  else
    "$@"
  fi
}

run_rust cargo fmt --all -- --check
run_rust cargo clippy --workspace --all-targets -- -D warnings
run_rust cargo test --workspace
npm run check:tauri-security
npm run check
npm run test
git diff --check
