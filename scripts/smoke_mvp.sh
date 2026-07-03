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

tmp_root="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_root"
}
trap cleanup EXIT

repo="$tmp_root/repo"
home="$tmp_root/home"
index_dir="$tmp_root/indexes"
mkdir -p "$home"
bash scripts/create_mvp_demo_repo.sh "$repo" >/dev/null

run_rust cargo build -p c4lens-cli
cli="./target/debug/c4lens"

run_cli() {
  HOME="$home" C4LENS_INDEX_DIR="$index_dir" "$cli" "$@"
}

init_repo="$tmp_root/init-repo"
mkdir -p "$init_repo"
init_json="$tmp_root/init.json"
run_cli init --repo "$init_repo" --name "Smoke Initialized Repo" --json > "$init_json"
node -e 'const payload = require(process.argv[1]); if (!payload.ok || payload.modelPath !== "c4/model.yml" || payload.schemaPath !== "c4/schema.json") { throw new Error(`unexpected init payload ${JSON.stringify(payload)}`); }' "$init_json"
run_cli validate --repo "$init_repo" --json >/dev/null
printf '%s\n' '{"title":"stale"}' > "$init_repo/c4/schema.json"
schema_json="$tmp_root/schema.json"
run_cli schema --repo "$init_repo" --json > "$schema_json"
node -e 'const payload = require(process.argv[1]); if (!payload.ok || payload.schemaPath !== "c4/schema.json") { throw new Error(`unexpected schema payload ${JSON.stringify(payload)}`); }' "$schema_json"
node -e 'const schema = require(process.argv[1]); if (schema.title !== "c4lens model" || schema.$id !== "https://c4lens.local/schema.json") { throw new Error("schema refresh did not restore bundled schema"); }' "$init_repo/c4/schema.json"
doctor_json="$tmp_root/init-doctor.json"
run_cli doctor --repo "$init_repo" --json > "$doctor_json"
node -e 'const payload = require(process.argv[1]); if (!payload.ok || !payload.model?.exists || !payload.schema?.exists) { throw new Error(`unexpected doctor payload ${JSON.stringify(payload)}`); }' "$doctor_json"

run_cli validate --repo "$repo" --json >/dev/null
scan_json="$tmp_root/scan.json"
run_cli scan --repo "$repo" --json > "$scan_json"
node -e 'const summary = require(process.argv[1]); if (summary.symbols < 10 || summary.imports < 5) { throw new Error(`expected multi-language symbols/imports, got ${summary.symbols}/${summary.imports}`); }' "$scan_json"
repo_id="$(node -e 'const summary = require(process.argv[1]); process.stdout.write(summary.repo?.id ?? "");' "$scan_json")"
if [[ -z "$repo_id" ]]; then
  printf '%s\n' "Unable to resolve smoke repo id from scan output." >&2
  exit 1
fi

lock_dir="$home/Library/Application Support/c4lens/locks"
lock_path="$lock_dir/$repo_id.write.lock"
mkdir -p "$lock_dir"
printf '%s' "smoke-mvp" > "$lock_path"
locked_scan_json="$tmp_root/locked-scan.json"
if run_cli scan --repo "$repo" --json > "$locked_scan_json"; then
  printf '%s\n' "Expected scan to fail while repository write lock is held." >&2
  exit 1
fi
node -e 'const payload = require(process.argv[1]); const code = payload.issues?.[0]?.code; if (code !== "repo.write_locked") { throw new Error(`expected repo.write_locked, got ${code}`); }' "$locked_scan_json"
locked_generate_json="$tmp_root/locked-generate.json"
if run_cli generate --repo "$repo" --write --json > "$locked_generate_json"; then
  printf '%s\n' "Expected generate --write to fail while repository write lock is held." >&2
  exit 1
fi
node -e 'const payload = require(process.argv[1]); const code = payload.issues?.[0]?.code; if (code !== "repo.write_locked") { throw new Error(`expected repo.write_locked, got ${code}`); }' "$locked_generate_json"
rm -f "$lock_path"

generated_json="$tmp_root/generated.json"
run_cli generate --repo "$repo" --scan --json > "$generated_json"
node -e 'const payload = require(process.argv[1]); const count = (payload.generatedYaml.match(/description: Imports/g) || []).length; if (count < 5) { throw new Error(`expected at least 5 generated import relationships, got ${count}`); }' "$generated_json"
run_cli generate --repo "$repo" --write >/dev/null
run_cli generate --repo "$repo" --check --json >/dev/null
run_cli doctor --repo "$repo" --json >/dev/null
test -f "$repo/c4/model.generated.yml"
test -f "$repo/c4/schema.json"

npm --workspace app run test -- App.e2e.test.tsx

printf '%s\n' "MVP smoke passed"
