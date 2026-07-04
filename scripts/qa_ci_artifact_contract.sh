#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

usage() {
  printf '%s\n' "Usage: npm run qa:ci-artifact -- <workflow-run-id> <commit-sha>" >&2
}

run_id="${1:-}"
commit_sha="${2:-}"

if [[ -z "$run_id" || -z "$commit_sha" ]]; then
  usage
  exit 64
fi

if ! command -v gh >/dev/null 2>&1; then
  printf '%s\n' "gh is required to verify CI artifact metadata." >&2
  exit 69
fi

tmp_json="$(mktemp "${TMPDIR:-/tmp}/c4lens-ci-artifacts.XXXXXX")"
trap 'rm -f "$tmp_json"' EXIT

gh api "repos/fuentesjr/c4lens/actions/runs/${run_id}/artifacts" >"$tmp_json"

node - "$tmp_json" "$commit_sha" <<'NODE'
const fs = require("node:fs");

const [jsonPath, commitSha] = process.argv.slice(2);
const artifactPayload = JSON.parse(fs.readFileSync(jsonPath, "utf8"));
const tauriConfig = JSON.parse(
  fs.readFileSync("crates/c4lens-tauri/tauri.conf.json", "utf8"),
);
const version = tauriConfig.version;
const expectedName = `c4lens-${version}-macos-universal-${commitSha}`;
const artifact = artifactPayload.artifacts.find(
  (candidate) => candidate.name === expectedName,
);

if (!artifact) {
  console.error(`Missing expected artifact: ${expectedName}`);
  console.error(
    `Available artifacts: ${
      artifactPayload.artifacts.map((candidate) => candidate.name).join(", ") ||
      "(none)"
    }`,
  );
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

console.log(`CI artifact contract passed: ${artifact.name}`);
console.log(`Expires at: ${artifact.expires_at}`);
console.log(`Size: ${artifact.size_in_bytes} bytes`);
NODE
