#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

failures=0

require_contains() {
  local path="$1"
  local expected="$2"

  if ! grep -Fq "$expected" "$path"; then
    printf '%s\n' "Missing required MVP doc text in $path:" >&2
    printf '  %s\n' "$expected" >&2
    failures=1
  fi
}

require_contains README.md "npm run check:mvp-docs"
require_contains README.md "npm run check:release-metadata"
require_contains README.md "c4lens init --repo /path/to/repo --name \"My System\""
require_contains README.md "c4lens schema --repo /path/to/repo"
require_contains README.md "c4lens doctor --repo /path/to/repo"
require_contains README.md "npm run demo:mvp-repo -- /tmp/c4lens-mvp-demo"
require_contains README.md "[Project tracker](PROJECT_TRACKER.md)"
require_contains README.md "[CLI quickstart](docs/cli-quickstart.md)"
require_contains README.md "[MVP first-run walkthrough](docs/mvp-first-run-walkthrough.md)"
require_contains README.md "[MVP manual QA template](docs/mvp-manual-qa-template.md)"
require_contains PROJECT_TRACKER.md "## Current Status"
require_contains PROJECT_TRACKER.md "## In Flight"
require_contains PROJECT_TRACKER.md "## Next Candidate Tasks"
require_contains docs/roadmap.md "| SVG/PDF/PNG export | Implemented |"
require_contains docs/roadmap.md "| CLI repo initialization | Implemented |"
require_contains docs/roadmap.md "| CLI schema refresh | Implemented |"
require_contains docs/roadmap.md "| CLI repo doctor | Implemented |"
require_contains docs/roadmap.md "| Release metadata contract check | Implemented |"
require_contains docs/roadmap.md "| CLI and renderer version visibility | Implemented |"
require_contains docs/roadmap.md "| macOS release manifest | Implemented |"
require_contains docs/roadmap.md "| MVP demo repository fixture | Implemented |"
require_contains docs/roadmap.md "| CLI onboarding smoke coverage | Implemented |"
require_contains docs/roadmap.md "| MVP documentation contract check | Implemented |"
require_contains docs/roadmap.md "| CLI quickstart | Implemented |"
require_contains docs/roadmap.md "| MVP first-run walkthrough | Implemented |"
require_contains docs/roadmap.md "| MVP manual QA template | Implemented |"
require_contains docs/roadmap.md "| Internal MVP release notes | Implemented |"
require_contains docs/mvp-release-checklist.md "Export SVG, PDF, and PNG from the current view."
require_contains docs/mvp-release-checklist.md "[MVP first-run walkthrough](mvp-first-run-walkthrough.md)"
require_contains docs/mvp-release-checklist.md "[MVP manual QA template](mvp-manual-qa-template.md)"
require_contains docs/mvp-release-checklist.md "CLI init, schema refresh, and doctor checks"
require_contains docs/mvp-release-checklist.md "doctor checks"
require_contains docs/mvp-release-checklist.md "c4lens init --repo <repo> --name \"Manual Smoke\""
require_contains docs/mvp-release-checklist.md "c4lens schema --repo <repo>"
require_contains docs/mvp-release-checklist.md "c4lens doctor --repo <repo>"
require_contains docs/mvp-release-checklist.md "Release metadata contract check."
require_contains docs/mvp-release-checklist.md "release-manifest.json"
require_contains docs/mvp-release-checklist.md "MVP documentation contract check."
require_contains docs/mvp-release-checklist.md "npm run demo:mvp-repo -- /tmp/c4lens-mvp-demo"
require_contains docs/mvp-release-notes.md "Version: 0.1.0"
require_contains docs/mvp-release-notes.md "c4lens init"
require_contains docs/mvp-release-notes.md "c4lens schema"
require_contains docs/mvp-release-notes.md "c4lens doctor"
require_contains docs/mvp-release-notes.md "c4lens --version"
require_contains docs/mvp-release-notes.md "release-manifest.json"
require_contains docs/mvp-release-notes.md "SVG, PDF, and PNG export"
require_contains docs/cli-quickstart.md "c4lens init --repo /path/to/repo --name \"My System\""
require_contains docs/cli-quickstart.md "c4lens doctor --repo /path/to/repo"
require_contains docs/cli-quickstart.md "c4lens scan --repo /path/to/repo --json"
require_contains docs/cli-quickstart.md "c4lens generate --repo /path/to/repo --write"
require_contains docs/cli-quickstart.md "c4lens validate --repo /path/to/repo"
require_contains docs/mvp-first-run-walkthrough.md "npm run demo:mvp-repo -- /tmp/c4lens-mvp-demo"
require_contains docs/mvp-first-run-walkthrough.md "target/debug/c4lens generate --repo /tmp/c4lens-mvp-demo --write"
require_contains docs/mvp-first-run-walkthrough.md "Export SVG, PDF, and PNG from the current view."
require_contains docs/mvp-manual-qa-template.md "Candidate commit:"
require_contains docs/mvp-manual-qa-template.md "Export SVG/PDF/PNG succeeds"
require_contains docs/spec/c4lens-desktop-spec.md 'format: "svg" | "pdf" | "png";'
require_contains docs/spec/c4lens-desktop-spec.md "pdfBase64?: string;"
require_contains docs/spec/c4lens-desktop-spec.md "c4lens doctor [--repo PATH] [--json]"
require_contains docs/spec/c4lens-desktop-spec.md "c4lens init [--repo PATH] [--name NAME] [--json]"
require_contains docs/spec/c4lens-desktop-spec.md "c4lens schema [--repo PATH] [--json]"
require_contains docs/design/c4lens-desktop-design.md "SVG/PDF/PNG"

if [[ "$failures" -ne 0 ]]; then
  exit 1
fi

printf '%s\n' "MVP docs check passed"
