#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

usage() {
  printf '%s\n' \
    "Usage: npm run qa:prepare-ci-candidate -- <workflow-run-id> <commit-sha> [output-root]" >&2
}

run_id="${1:-}"
commit_sha="${2:-}"
output_root="${3:-target/mvp-candidates}"

if [[ -z "$run_id" || -z "$commit_sha" ]]; then
  usage
  exit 64
fi

if ! command -v gh >/dev/null 2>&1; then
  printf '%s\n' "gh is required to download CI candidate artifacts." >&2
  exit 69
fi

version="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").version')"
artifact_name="c4lens-${version}-macos-universal-${commit_sha}"
candidate_root="$output_root/$artifact_name"

bash scripts/qa_ci_artifact_contract.sh "$run_id" "$commit_sha" >/dev/null

manifest_path=""

if [[ -d "$candidate_root" ]]; then
  manifest_path="$(find "$candidate_root" -type f -name release-manifest.json -print -quit)"
  if [[ -z "$manifest_path" ]]; then
    printf 'Candidate output exists but is incomplete: %s\n' "$candidate_root" >&2
    printf '%s\n' "Choose a new output root or remove the incomplete candidate directory." >&2
    exit 1
  fi
else
  if [[ -e "$candidate_root" ]]; then
    printf 'Candidate output exists and is not a directory: %s\n' "$candidate_root" >&2
    exit 1
  fi

  mkdir -p "$candidate_root"

  gh run download "$run_id" \
    --repo fuentesjr/c4lens \
    --name "$artifact_name" \
    --dir "$candidate_root"

  manifest_path="$(find "$candidate_root" -type f -name release-manifest.json -print -quit)"
fi

if [[ -z "$manifest_path" ]]; then
  printf 'Downloaded candidate is missing release-manifest.json under %s\n' "$candidate_root" >&2
  exit 1
fi

bundle_root="$(dirname "$manifest_path")"

bash scripts/verify_macos_artifacts.sh "$bundle_root" >/dev/null

if [[ "$(uname -s)" == "Darwin" ]]; then
  bash scripts/qa_installed_macos_artifact.sh "$bundle_root" >/dev/null
else
  printf 'Skipping installed macOS artifact QA on %s.\n' "$(uname -s)" >&2
fi

product_name="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").productName')"
dmg_path="$(find "$bundle_root" -type f -path '*/dmg/*.dmg' ! -name 'rw.*.dmg' -print -quit)"
app_path="$(find "$bundle_root" -type d -path "*/macos/${product_name}.app" -print -quit)"

printf '%s\n' "Prepared CI candidate artifact"
printf '  run: %s\n' "$run_id"
printf '  commit: %s\n' "$commit_sha"
printf '  artifact: %s\n' "$artifact_name"
printf '  candidate root: %s\n' "$candidate_root"
printf '  bundle root: %s\n' "$bundle_root"
printf '  app: %s\n' "$app_path"
printf '  dmg: %s\n' "$dmg_path"
printf '  manifest: %s\n' "$manifest_path"
