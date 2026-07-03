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
require_contains docs/roadmap.md "| SVG/PDF/PNG export | Implemented |"
require_contains docs/roadmap.md "| MVP documentation contract check | Implemented |"
require_contains docs/mvp-release-checklist.md "Export SVG, PDF, and PNG from the current view."
require_contains docs/mvp-release-checklist.md "MVP documentation contract check."
require_contains docs/spec/c4lens-desktop-spec.md 'format: "svg" | "pdf" | "png";'
require_contains docs/spec/c4lens-desktop-spec.md "pdfBase64?: string;"
require_contains docs/design/c4lens-desktop-design.md "SVG/PDF/PNG"

if [[ "$failures" -ne 0 ]]; then
  exit 1
fi

printf '%s\n' "MVP docs check passed"
