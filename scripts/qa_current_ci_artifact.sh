#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

commit_sha="${1:-$(git rev-parse HEAD)}"
branch="${C4LENS_CI_BRANCH:-$(git rev-parse --abbrev-ref HEAD)}"
workflow_name="${C4LENS_CI_WORKFLOW:-CI}"

if ! command -v gh >/dev/null 2>&1; then
  printf '%s\n' "gh is required to verify current CI artifact metadata." >&2
  exit 69
fi

tmp_json="$(mktemp "${TMPDIR:-/tmp}/c4lens-ci-runs.XXXXXX")"
trap 'rm -f "$tmp_json"' EXIT

gh run list \
  --repo fuentesjr/c4lens \
  --branch "$branch" \
  --limit 20 \
  --json databaseId,status,conclusion,headSha,workflowName,displayTitle,url,createdAt \
  >"$tmp_json"

run_id="$(
  node - "$tmp_json" "$commit_sha" "$workflow_name" <<'NODE'
const fs = require("node:fs");

const [jsonPath, commitSha, workflowName] = process.argv.slice(2);
const runs = JSON.parse(fs.readFileSync(jsonPath, "utf8"));
const run = runs.find(
  (candidate) =>
    candidate.headSha === commitSha && candidate.workflowName === workflowName,
);

if (!run) {
  console.error(`No ${workflowName} run found for ${commitSha}.`);
  process.exit(1);
}

console.error(`CI run found: ${run.databaseId} (${run.status}/${run.conclusion ?? "none"})`);
console.error(run.url);

if (run.status !== "completed") {
  console.error(`CI run is not complete yet: ${run.status}`);
  process.exit(1);
}

if (run.conclusion !== "success") {
  console.error(`CI run did not succeed: ${run.conclusion ?? "none"}`);
  process.exit(1);
}

console.log(run.databaseId);
NODE
)"

bash scripts/qa_ci_artifact_contract.sh "$run_id" "$commit_sha"
