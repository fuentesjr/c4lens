#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

usage() {
  printf '%s\n' \
    "Usage: npm run qa:ready-for-human -- <workflow-run-id> <commit-sha> [date]" >&2
}

run_id="${1:-}"
commit_sha="${2:-}"
packet_date="${3:-$(date +%F)}"

if [[ -z "$run_id" || -z "$commit_sha" ]]; then
  usage
  exit 64
fi

short_sha="${commit_sha:0:7}"

bash scripts/qa_ci_artifact_contract.sh "$run_id" "$commit_sha" >/dev/null
bash scripts/write_ci_artifact_log.sh \
  "$run_id" \
  "$commit_sha" \
  "docs/qa/ci-artifact-${short_sha}-${packet_date}.md"
bash scripts/prepare_ci_candidate.sh "$run_id" "$commit_sha"
bash scripts/write_mvp_installed_gui_handoff.sh \
  "$run_id" \
  "$commit_sha" \
  "docs/qa/mvp-installed-gui-${packet_date}.md"
bash scripts/write_mvp_manual_qa_stub.sh \
  "$run_id" \
  "$commit_sha" \
  "docs/qa/mvp-manual-qa-${short_sha}-${packet_date}.md"
bash scripts/check_mvp_candidate_packet.sh "$run_id" "$commit_sha" "$packet_date"

printf '%s\n' "MVP candidate is ready for human installed-app QA"
printf '  run: %s\n' "$run_id"
printf '  commit: %s\n' "$commit_sha"
printf '  artifact log: docs/qa/ci-artifact-%s-%s.md\n' "$short_sha" "$packet_date"
printf '  GUI handoff: docs/qa/mvp-installed-gui-%s.md\n' "$packet_date"
printf '  manual QA stub: docs/qa/mvp-manual-qa-%s-%s.md\n' "$short_sha" "$packet_date"
