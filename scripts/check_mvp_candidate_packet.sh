#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

usage() {
  printf '%s\n' \
    "Usage: npm run qa:candidate-packet -- <workflow-run-id> <commit-sha> [date] [candidate-root]" >&2
}

run_id="${1:-}"
commit_sha="${2:-}"
packet_date="${3:-$(date +%F)}"

if [[ -z "$run_id" || -z "$commit_sha" ]]; then
  usage
  exit 64
fi

version="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").version')"
short_sha="${commit_sha:0:7}"
artifact_name="c4lens-${version}-macos-universal-${commit_sha}"
candidate_root="${4:-target/mvp-candidates/$artifact_name}"
artifact_log="docs/qa/ci-artifact-${short_sha}-${packet_date}.md"
manual_stub="docs/qa/mvp-manual-qa-${short_sha}-${packet_date}.md"
gui_handoff="docs/qa/mvp-installed-gui-${packet_date}.md"

failures=0

require_file() {
  local path="$1"

  if [[ ! -e "$path" ]]; then
    printf 'Missing candidate packet file: %s\n' "$path" >&2
    failures=1
  fi
}

require_contains() {
  local path="$1"
  local expected="$2"

  if [[ ! -e "$path" ]]; then
    return
  fi

  if ! grep -Fq -- "$expected" "$path"; then
    printf 'Missing candidate packet text in %s:\n' "$path" >&2
    printf '  %s\n' "$expected" >&2
    failures=1
  fi
}

require_file "$artifact_log"
require_file "$manual_stub"
require_file "$gui_handoff"
require_file "$candidate_root/release-manifest.json"
require_file "$candidate_root/dmg/c4lens_${version}_universal.dmg"
require_file "$candidate_root/macos/c4lens.app"

require_contains "$artifact_log" "$run_id"
require_contains "$artifact_log" "$commit_sha"
require_contains "$artifact_log" "$artifact_name"
require_contains "$artifact_log" "npm run qa:prepare-ci-candidate -- $run_id $commit_sha"

require_contains "$manual_stub" "$run_id"
require_contains "$manual_stub" "$commit_sha"
require_contains "$manual_stub" "$artifact_name"
require_contains "$manual_stub" "Human installed-app GUI pass completed."
require_contains "$manual_stub" "Export SVG/PDF/PNG succeeds"

require_contains "$gui_handoff" "$run_id"
require_contains "$gui_handoff" "$commit_sha"
require_contains "$gui_handoff" "$artifact_name"
require_contains "$gui_handoff" "ready for human installed-app interaction pass"

if [[ "$failures" -ne 0 ]]; then
  exit 1
fi

bash scripts/verify_macos_artifacts.sh "$candidate_root" >/dev/null

printf '%s\n' "MVP candidate packet check passed"
printf '  run: %s\n' "$run_id"
printf '  commit: %s\n' "$commit_sha"
printf '  artifact: %s\n' "$artifact_name"
printf '  artifact log: %s\n' "$artifact_log"
printf '  manual QA stub: %s\n' "$manual_stub"
printf '  GUI handoff: %s\n' "$gui_handoff"
printf '  candidate root: %s\n' "$candidate_root"
