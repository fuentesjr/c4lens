#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

usage() {
  printf '%s\n' \
    "Usage: npm run qa:manual-stub -- <workflow-run-id> <commit-sha> [output-path]" >&2
}

run_id="${1:-}"
commit_sha="${2:-}"
short_sha="${commit_sha:0:7}"
output_path="${3:-docs/qa/mvp-manual-qa-${short_sha}-$(date +%F).md}"

if [[ -z "$run_id" || -z "$commit_sha" ]]; then
  usage
  exit 64
fi

if ! command -v gh >/dev/null 2>&1; then
  printf '%s\n' "gh is required to write the manual QA stub." >&2
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

const outputDate =
  path.basename(outputPath).match(/\d{4}-\d{2}-\d{2}/)?.[0] ??
  new Date().toISOString().slice(0, 10);
const candidateRoot = `target/mvp-candidates/${artifact.name}`;
const markdown = `# MVP Manual QA - ${shortSha} - ${outputDate}

Use this file to record the remaining human installed-app GUI pass for the
current internal macOS MVP candidate.

## Candidate

- Tester:
- Date:
- Machine:
- macOS version:
- Candidate version: \`${version}\`
- Candidate commit: \`${commitSha}\`
- Artifact source: GitHub Actions workflow run \`${runId}\`
- Artifact name:
  \`${artifact.name}\`
- Workflow run: \`${runId}\`
- App path:
  \`${candidateRoot}/macos/c4lens.app\`
- DMG path:
  \`${candidateRoot}/dmg/c4lens_${version}_universal.dmg\`
- \`release-manifest.json\` path:
  \`${candidateRoot}/release-manifest.json\`

## Automated Gate

- [x] CI run \`${runId}\` completed successfully.
- [x] \`npm run qa:current-ci-artifact -- ${commitSha}\` passed.
- [x] \`npm run qa:prepare-ci-candidate -- ${runId} ${commitSha}\` passed.
- [x] \`npm run qa:candidate-packet -- ${runId} ${commitSha}\` passed.
- [ ] Human installed-app GUI pass completed.

Notes:

\`\`\`text
The candidate is downloaded and verified under target/mvp-candidates/.
The remaining gate requires Finder/app interaction from an installed candidate.
\`\`\`

## Manual Results

| Area | Result | Notes |
| --- | --- | --- |
| Install from DMG | Not run | Requires human GUI session. |
| Launch installed \`c4lens.app\` | Not run | Requires human GUI session. |
| Status bar shows expected version | Not run | Expected \`${version}\`. |
| Open local repository | Not run | Use \`/tmp/c4lens-mvp-demo\` or equivalent. |
| \`c4lens init\` creates \`c4/model.yml\` and \`c4/schema.json\` | Not run | |
| \`c4lens schema\` restores bundled editor schema | Not run | |
| \`c4lens doctor\` reports repository readiness | Not run | |
| Validate valid model | Not run | |
| Invalid model keeps last valid canvas and shows path/line/column details | Not run | |
| Scan updates source counts | Not run | |
| Generate review/apply writes \`c4/model.generated.yml\` | Not run | |
| Generated provenance is visible | Not run | |
| Search opens elements, files, and symbols | Not run | |
| Jump to code opens source location | Not run | |
| Export SVG/PDF/PNG succeeds | Not run | |
| Light and dark themes render correctly | Not run | |
| Minimum window size remains usable | Not run | |
| \`c4lens --version\` matches app version | Not run | |

## Blockers

List any issue that should prevent sharing the candidate:

\`\`\`text
None recorded yet. Human installed-app GUI pass has not been run.
\`\`\`

Classify findings with [MVP QA triage](../mvp-qa-triage.md).

## Follow-Ups

List non-blocking issues or release-note clarifications:

\`\`\`text
None recorded yet.
\`\`\`
`;

fs.mkdirSync(path.dirname(outputPath), { recursive: true });
fs.writeFileSync(outputPath, markdown);
console.log(`Wrote ${outputPath}`);
NODE
