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

reject_contains() {
  local path="$1"
  local unexpected="$2"

  if [[ ! -e "$path" ]]; then
    return
  fi

  if grep -Fq -- "$unexpected" "$path"; then
    printf 'Unexpected candidate packet text in %s:\n' "$path" >&2
    printf '  %s\n' "$unexpected" >&2
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
require_contains "$manual_stub" "- [ ] Human installed-app GUI pass completed."
require_contains "$manual_stub" "| Install from DMG | Not run | Requires human GUI session. |"
require_contains "$manual_stub" "| Launch installed \`c4lens.app\` | Not run | Requires human GUI session. |"
require_contains "$manual_stub" "| Status bar shows expected version | Not run | Expected \`$version\`. |"
require_contains "$manual_stub" "| Open local repository | Not run | Use \`/tmp/c4lens-mvp-demo\` or equivalent. |"
require_contains "$manual_stub" "| \`c4lens init\` creates \`c4/model.yml\` and \`c4/schema.json\` | Not run | |"
require_contains "$manual_stub" "| \`c4lens schema\` restores bundled editor schema | Not run | |"
require_contains "$manual_stub" "| \`c4lens doctor\` reports repository readiness | Not run | |"
require_contains "$manual_stub" "| Validate valid model | Not run | |"
require_contains "$manual_stub" "| Invalid model keeps last valid canvas and shows path/line/column details | Not run | |"
require_contains "$manual_stub" "| Scan updates source counts | Not run | |"
require_contains "$manual_stub" "| Generate review/apply writes \`c4/model.generated.yml\` | Not run | |"
require_contains "$manual_stub" "| Generated provenance is visible | Not run | |"
require_contains "$manual_stub" "| Search opens elements, files, and symbols | Not run | |"
require_contains "$manual_stub" "| Jump to code opens source location | Not run | |"
require_contains "$manual_stub" "| Export SVG/PDF/PNG succeeds | Not run | |"
require_contains "$manual_stub" "| Light and dark themes render correctly | Not run | |"
require_contains "$manual_stub" "| Minimum window size remains usable | Not run | |"
require_contains "$manual_stub" "| \`c4lens --version\` matches app version | Not run | |"
reject_contains "$manual_stub" "- [x] Human installed-app GUI pass completed."
reject_contains "$manual_stub" "| Pass |"
reject_contains "$manual_stub" "| Fail |"

require_contains "$gui_handoff" "$run_id"
require_contains "$gui_handoff" "$commit_sha"
require_contains "$gui_handoff" "$artifact_name"
require_contains "$gui_handoff" "ready for human installed-app interaction pass"
require_contains "$gui_handoff" "| DMG mounts in Finder | Not run | Requires human GUI session. |"
require_contains "$gui_handoff" "| \`c4lens.app\` copies to Applications or a temporary install directory | Not run | Requires human GUI session. |"
require_contains "$gui_handoff" "| App launches from installed location | Not run | Requires human GUI session. |"
require_contains "$gui_handoff" "| Status bar shows version \`$version\` | Not run | Requires human GUI session. |"
require_contains "$gui_handoff" "| Demo repository opens successfully | Not run | Use \`/tmp/c4lens-mvp-demo\` or equivalent. |"
require_contains "$gui_handoff" "| Scan updates source counts | Not run | Requires app interaction. |"
require_contains "$gui_handoff" "| Generate preview, diff review, and apply succeed | Not run | Confirm \`c4/model.generated.yml\` is written. |"
require_contains "$gui_handoff" "| Search opens an element, file, and symbol | Not run | Exercise keyboard navigation. |"
require_contains "$gui_handoff" "| Jump to code opens source location | Not run | Requires installed app permissions. |"
require_contains "$gui_handoff" "| Export SVG/PDF/PNG succeeds | Not run | Verify saved files exist and are non-empty. |"
require_contains "$gui_handoff" "| Light/dark theme toggle persists during session | Not run | Requires app interaction. |"
require_contains "$gui_handoff" "| Minimum-size resize remains usable | Not run | Requires app interaction. |"
reject_contains "$gui_handoff" "| Pass |"
reject_contains "$gui_handoff" "| Fail |"

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
