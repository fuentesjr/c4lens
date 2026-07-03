#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

usage() {
  printf 'Usage: npm run qa:first-run -- [destination]\n' >&2
}

if [[ "$#" -gt 1 ]]; then
  usage
  exit 64
fi

if [[ "$#" -eq 1 ]]; then
  repo="$1"
else
  repo="$(mktemp -d "${TMPDIR:-/tmp}/c4lens-first-run-repo.XXXXXX")"
fi

if [[ -e "$repo" && ! -d "$repo" ]]; then
  printf 'Destination exists and is not a directory: %s\n' "$repo" >&2
  exit 1
fi

if [[ -d "$repo" && -n "$(find "$repo" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  printf 'Destination directory must be empty: %s\n' "$repo" >&2
  exit 1
fi

qa_state="$(mktemp -d "${TMPDIR:-/tmp}/c4lens-first-run-state.XXXXXX")"
qa_home="$qa_state/home"
qa_index="$qa_state/indexes"
mkdir -p "$qa_home" "$qa_index"

run_rust() {
  if command -v mise >/dev/null 2>&1; then
    mise exec rust@1.96.0 -- "$@"
  else
    "$@"
  fi
}

bash scripts/create_mvp_demo_repo.sh "$repo" >/dev/null
run_rust cargo build -p c4lens-cli >/dev/null

cli="./target/debug/c4lens"

run_cli() {
  HOME="$qa_home" C4LENS_INDEX_DIR="$qa_index" "$cli" "$@"
}

doctor_before_schema="$qa_state/doctor-before-schema.json"
if run_cli doctor --repo "$repo" --json > "$doctor_before_schema"; then
  printf '%s\n' "Expected doctor to request schema refresh before first-run setup was complete." >&2
  exit 1
fi
node -e 'const payload = require(process.argv[1]); if (payload.ok !== false || payload.schema?.exists !== false) { throw new Error(`unexpected pre-schema doctor payload ${JSON.stringify(payload)}`); }' "$doctor_before_schema"

schema_json="$qa_state/schema.json"
run_cli schema --repo "$repo" --json > "$schema_json"
node -e 'const payload = require(process.argv[1]); if (!payload.ok || payload.schemaPath !== "c4/schema.json") { throw new Error(`unexpected schema payload ${JSON.stringify(payload)}`); }' "$schema_json"

doctor_after_schema="$qa_state/doctor-after-schema.json"
run_cli doctor --repo "$repo" --json > "$doctor_after_schema"
node -e 'const payload = require(process.argv[1]); if (!payload.ok || !payload.schema?.exists || !payload.model?.exists) { throw new Error(`unexpected ready doctor payload ${JSON.stringify(payload)}`); }' "$doctor_after_schema"

run_cli validate --repo "$repo" --json >/dev/null

scan_json="$qa_state/scan.json"
run_cli scan --repo "$repo" --json > "$scan_json"
node -e 'const summary = require(process.argv[1]); if (summary.symbols < 10 || summary.imports < 5) { throw new Error(`expected demo symbols/imports, got ${summary.symbols}/${summary.imports}`); }' "$scan_json"

generated_json="$qa_state/generated.json"
run_cli generate --repo "$repo" --scan --json > "$generated_json"
node -e 'const payload = require(process.argv[1]); const count = (payload.generatedYaml.match(/description: Imports/g) || []).length; if (!payload.ok || count < 5) { throw new Error(`expected at least 5 generated import relationships, got ${count}`); }' "$generated_json"

run_cli generate --repo "$repo" --scan --write >/dev/null
run_cli generate --repo "$repo" --check --json >/dev/null
run_cli validate --repo "$repo" --json >/dev/null

test -f "$repo/c4/model.generated.yml"
test -f "$repo/c4/schema.json"

printf '%s\n' "MVP first-run CLI QA passed"
printf '  repo: %s\n' "$repo"
printf '  state: %s\n' "$qa_state"
