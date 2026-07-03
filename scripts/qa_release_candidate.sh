#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

bundle_root="${1:-target/universal-apple-darwin/release/bundle}"

printf '%s\n' "Running MVP first-run CLI QA..."
bash scripts/qa_first_run_cli.sh

if [[ "$(uname -s)" == "Darwin" ]]; then
  printf '%s\n' "Running installed macOS artifact QA..."
  bash scripts/qa_installed_macos_artifact.sh "$bundle_root"
else
  printf 'Skipping installed macOS artifact QA on %s.\n' "$(uname -s)" >&2
fi

printf '%s\n' "Running MVP documentation contract check..."
bash scripts/check_mvp_docs.sh

printf '%s\n' "MVP release-candidate QA passed"
