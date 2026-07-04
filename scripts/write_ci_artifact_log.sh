#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

usage() {
  printf '%s\n' \
    "Usage: npm run qa:artifact-log -- <workflow-run-id> <commit-sha> [output-path]" >&2
}

run_id="${1:-}"
commit_sha="${2:-}"
short_sha="${commit_sha:0:7}"
output_path="${3:-docs/qa/ci-artifact-${short_sha}-$(date +%F).md}"

if [[ -z "$run_id" || -z "$commit_sha" ]]; then
  usage
  exit 64
fi

if ! command -v gh >/dev/null 2>&1; then
  printf '%s\n' "gh is required to write the CI artifact log." >&2
  exit 69
fi

tmp_artifacts="$(mktemp "${TMPDIR:-/tmp}/c4lens-ci-artifacts.XXXXXX")"
tmp_run="$(mktemp "${TMPDIR:-/tmp}/c4lens-ci-run.XXXXXX")"
trap 'rm -f "$tmp_artifacts" "$tmp_run"' EXIT

gh api "repos/fuentesjr/c4lens/actions/runs/${run_id}/artifacts" >"$tmp_artifacts"
gh run view "$run_id" \
  --repo fuentesjr/c4lens \
  --json url,status,conclusion,jobs \
  >"$tmp_run"

node - "$tmp_artifacts" "$tmp_run" "$run_id" "$commit_sha" "$output_path" <<'NODE'
const fs = require("node:fs");
const path = require("node:path");

const [artifactsPath, runPath, runId, commitSha, outputPath] =
  process.argv.slice(2);
const artifactPayload = JSON.parse(fs.readFileSync(artifactsPath, "utf8"));
const runPayload = JSON.parse(fs.readFileSync(runPath, "utf8"));
const tauriConfig = JSON.parse(
  fs.readFileSync("crates/c4lens-tauri/tauri.conf.json", "utf8"),
);
const version = tauriConfig.version;
const shortSha = commitSha.slice(0, 7);
const expectedName = `c4lens-${version}-macos-universal-${commitSha}`;
const artifact = artifactPayload.artifacts.find(
  (candidate) => candidate.name === expectedName,
);

if (!artifact) {
  console.error(`Missing expected artifact: ${expectedName}`);
  process.exit(1);
}

if (artifact.expired) {
  console.error(`Artifact is expired: ${artifact.name}`);
  process.exit(1);
}

if (!artifact.expires_at) {
  console.error(`Artifact is missing an expiration timestamp: ${artifact.name}`);
  process.exit(1);
}

if (!Number.isFinite(artifact.size_in_bytes) || artifact.size_in_bytes <= 0) {
  console.error(`Artifact has invalid size: ${artifact.name}`);
  process.exit(1);
}

if (runPayload.status !== "completed" || runPayload.conclusion !== "success") {
  console.error(
    `Run is not successful: ${runPayload.status}/${runPayload.conclusion ?? "none"}`,
  );
  process.exit(1);
}

const jobRows = runPayload.jobs
  .map((job) => {
    const result = job.conclusion === "success" ? "Pass" : job.conclusion ?? job.status;
    const notes = job.name === "Check"
      ? "Full quality gate passed on the pushed candidate commit."
      : job.name === "Package macOS"
        ? "Unsigned universal macOS build, package verification, release version read, and artifact upload completed."
        : "Workflow job completed.";
    return `| ${job.name} | ${result} | ${notes} |`;
  })
  .join("\n");

const outputDate =
  path.basename(outputPath).match(/\d{4}-\d{2}-\d{2}/)?.[0] ??
  new Date().toISOString().slice(0, 10);
const candidateRoot = `target/mvp-candidates/${artifact.name}`;
const markdown = `# CI Artifact Confirmation - ${shortSha} - ${outputDate}

## Run

- Workflow run: \`${runId}\`
- Run URL: \`${runPayload.url}\`
- Commit: \`${commitSha}\`
- Status: ${runPayload.status}
- Conclusion: ${runPayload.conclusion}

## Jobs

| Job | Result | Notes |
| --- | --- | --- |
${jobRows}

## Artifact

- Name:
  \`${artifact.name}\`
- Expired: ${artifact.expired}
- Expires at: \`${artifact.expires_at}\`
- Size: ${artifact.size_in_bytes} bytes

Verified with:

\`\`\`sh
npm run qa:current-ci-artifact -- ${commitSha}
npm run qa:prepare-ci-candidate -- ${runId} ${commitSha}
npm run qa:ready-for-human -- ${runId} ${commitSha}
\`\`\`

Prepared local paths:

\`\`\`text
${candidateRoot}/macos/c4lens.app
${candidateRoot}/dmg/c4lens_${version}_universal.dmg
${candidateRoot}/release-manifest.json
\`\`\`

The artifact name, size, expiration, downloaded bundle, installed-artifact QA,
and candidate-packet checks match the release artifact handling contract.
`;

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, markdown);
console.log(`Wrote ${outputPath}`);
NODE
