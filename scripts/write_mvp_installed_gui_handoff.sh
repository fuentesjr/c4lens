#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

usage() {
  printf '%s\n' \
    "Usage: npm run qa:gui-handoff -- <workflow-run-id> <commit-sha> [output-path]" >&2
}

run_id="${1:-}"
commit_sha="${2:-}"
output_path="${3:-docs/qa/mvp-installed-gui-$(date +%F).md}"

if [[ -z "$run_id" || -z "$commit_sha" ]]; then
  usage
  exit 64
fi

if ! command -v gh >/dev/null 2>&1; then
  printf '%s\n' "gh is required to write the installed GUI handoff." >&2
  exit 69
fi

tmp_json="$(mktemp "${TMPDIR:-/tmp}/c4lens-ci-artifacts.XXXXXX")"
trap 'rm -f "$tmp_json"' EXIT

gh api "repos/fuentesjr/c4lens/actions/runs/${run_id}/artifacts" >"$tmp_json"

node - "$tmp_json" "$run_id" "$commit_sha" "$output_path" <<'NODE'
const fs = require("node:fs");
const path = require("node:path");

const [jsonPath, runId, commitSha, outputPath] = process.argv.slice(2);
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

const outputDate =
  path.basename(outputPath).match(/\d{4}-\d{2}-\d{2}/)?.[0] ??
  new Date().toISOString().slice(0, 10);

const markdown = `# MVP Installed GUI QA Handoff - ${outputDate}

Candidate commit: \`${commitSha}\`

Workflow run: \`${runId}\`

Artifact name:
\`${artifact.name}\`

Artifact expiration: \`${artifact.expires_at}\`

Artifact size: ${artifact.size_in_bytes} bytes

Status: ready for human installed-app interaction pass.

## Automated Gate Context

- CI run \`${runId}\` uploaded the expected macOS universal artifact.
- \`npm run qa:current-ci-artifact -- ${commitSha}\` verifies this run and
  artifact metadata.
- \`npm run qa:prepare-ci-candidate -- ${runId} ${commitSha}\` downloads and
  verifies a local candidate bundle under \`target/mvp-candidates/\`.
- \`npm run qa:release-candidate\` remains the local pre-human-review gate for
  first-run CLI QA, installed macOS artifact QA, and MVP docs contract checks.

## Human Interaction Checklist

Download the CI artifact or rebuild locally with \`npm run smoke:release\`,
install from the DMG on a current supported macOS machine, then record results
in \`docs/mvp-manual-qa-template.md\`.

| Workflow | Result | Notes |
| --- | --- | --- |
| DMG mounts in Finder | Not run | Requires human GUI session. |
| \`c4lens.app\` copies to Applications or a temporary install directory | Not run | Requires human GUI session. |
| App launches from installed location | Not run | Requires human GUI session. |
| Status bar shows version \`${version}\` | Not run | Requires human GUI session. |
| Demo repository opens successfully | Not run | Use \`/tmp/c4lens-mvp-demo\` or equivalent. |
| Scan updates source counts | Not run | Requires app interaction. |
| Generate preview, diff review, and apply succeed | Not run | Confirm \`c4/model.generated.yml\` is written. |
| Search opens an element, file, and symbol | Not run | Exercise keyboard navigation. |
| Jump to code opens source location | Not run | Requires installed app permissions. |
| Export SVG/PDF/PNG succeeds | Not run | Verify saved files exist and are non-empty. |
| Light/dark theme toggle persists during session | Not run | Requires app interaction. |
| Minimum-size resize remains usable | Not run | Requires app interaction. |

## Blocker/High Findings

None from automated gates. The human installed-app interaction pass has not yet
been run.
`;

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, markdown);
console.log(`Wrote ${outputPath}`);
NODE
