# Product Roadmap

This roadmap tracks visible product gaps separately from technical debt. Items
listed here are planned or deferred capabilities, not regressions.

## Phase 2 Backlog

| Item | Status | Notes |
| --- | --- | --- |
| Search | Implemented | Topbar search queries elements, indexed files, and indexed symbols with deterministic, bounded results. |
| SVG/PDF/PNG export | Implemented | The renderer serializes the current view and the Tauri backend saves SVG, PDF, or PNG through a native save dialog. |
| Layout caching | Implemented | ELK output is cached by source SHA, view scope, and stable layout input. Model/source/scope/input changes produce a new cache key. |
| Generation diff/review UX | Implemented | The app shows a diff summary, change list, generated YAML preview, and accept-all apply action. |
| Generated provenance badges | Implemented | Generated elements and relationships are visibly marked in the canvas and detail panel. |
| Dependency highlighting and focus mode | Implemented | Hovering or selecting a node highlights connected dependencies; Linked focus mode dims unrelated nodes. |
| macOS packaging | Ready | `npm run tauri:build:macos` builds unsigned universal macOS app and DMG artifacts for internal development. Signing/notarization remains a release step. |

## MVP Hardening

| Item | Status | Notes |
| --- | --- | --- |
| Persisted color theme | Implemented | The renderer stores the selected light/dark theme locally and reapplies it on startup. |
| Last repository restore | Implemented | The desktop shell remembers the last opened repository path and reopens it when no active model is already loaded. |
| Layout cache acceptance inputs | Implemented | ELK cache keys include layout options and measured node dimensions, not only model/source/scope data. |
| MVP smoke command | Implemented | `npm run smoke:mvp` validates the CLI path from temporary repo creation through scan, generate, write, drift check, and renderer E2E workflows. |
| CLI repo initialization | Implemented | `c4lens init` creates `c4/model.yml` and refreshes `c4/schema.json` without overwriting an authored model. |
| CLI schema refresh | Implemented | `c4lens schema` rewrites `c4/schema.json` from the bundled schema for editor autocomplete and schema drift repair. |
| CLI repo doctor | Implemented | `c4lens doctor` reports model/schema/generated-overlay presence, validation health, and setup recommendations without mutating the repo. |
| Roadmap and quality-gate docs | Implemented | README and roadmap now document the MVP hardening gate and remaining deferred work. |

## MVP Readiness

| Item | Status | Notes |
| --- | --- | --- |
| Code symbol detail panel | Implemented | Selected elements can show indexed symbols from their file or code directory and jump directly to a symbol location. |
| Scan progress events | Implemented | Desktop scan and generate-with-scan emit repo-scoped `scan-progress` events consumed by the renderer status bar. |
| Source change re-indexing | Implemented | The watcher detects non-control source edits, runs an incremental scan, and emits `index-updated`. |
| Schema drift repair | Implemented | Repos with stale `c4/schema.json` get a validation warning and a desktop repair action that refreshes the bundled schema. |
| GUI smoke coverage | Implemented | `npm run smoke:mvp` now includes the renderer E2E workflow tests in addition to the CLI smoke path. |
| Keyboardable global search | Implemented | Search results now support ArrowUp/ArrowDown, Enter, Escape, active-result styling, and combobox/listbox ARIA state. |
| Validation issue jump-to-file | Implemented | Validation cards show path, line, and column details when available and can open the issue location from the desktop shell. |

## MVP Release Readiness

| Item | Status | Notes |
| --- | --- | --- |
| TypeScript/JavaScript scanner coverage | Implemented | The scanner extracts best-effort classes, interfaces, enums, functions, constants, and import/require edges for JS/TS files. |
| Python scanner coverage | Implemented | The scanner extracts best-effort classes, functions, methods, constants, and import/from edges for Python files. |
| Ruby scanner coverage | Implemented | The scanner extracts best-effort modules, classes, methods, constants, and require/require_relative edges for Ruby files. |
| Go scanner coverage | Implemented | The scanner extracts best-effort structs, interfaces, functions, methods, constants, and import edges with `go.mod` module-prefix resolution. |
| CLI binary contract | Implemented | The CLI crate now builds the MVP `c4lens` executable name and smoke/tests invoke that binary. |
| Cross-language import relationship generation | Implemented | Generated component relationships now consume resolved internal imports from the full MVP language index, not only Rust crate imports. |
| Multi-language generation regression coverage | Implemented | CLI generation tests cover import-derived relationships for TypeScript, Python, Ruby, Go, and Rust. |
| Multi-language MVP smoke fixture | Implemented | `npm run smoke:mvp` now scans a mixed-language repo and checks generated import relationships across the MVP parser set. |
| CLI onboarding smoke coverage | Implemented | `npm run smoke:mvp` covers `c4lens init`, `c4lens schema`, `c4lens doctor`, validation, scan, generation, writer locks, and renderer E2E workflows. |
| PNG export workflow coverage | Implemented | Renderer E2E coverage now exercises PNG export payload generation and desktop IPC handoff. |
| PDF export workflow coverage | Implemented | Renderer E2E coverage now exercises PDF export payload generation and desktop IPC handoff. |
| macOS artifact verification | Implemented | `npm run package:verify` validates the unsigned app bundle, DMG, plist, executable, and universal architectures. |
| Release smoke command | Implemented | `npm run smoke:release` runs the quality gate, MVP smoke, unsigned macOS build, and artifact verification on macOS. |
| macOS packaging CI | Implemented | CI builds and uploads unsigned universal macOS artifacts on pushes to `main` and manual workflow dispatches. |
| Packaged-app release metadata smoke | Implemented | `npm run package:verify` now checks bundle name, identifier, version metadata, universal executable architecture, and DMG checksum validity. |
| Release metadata contract check | Implemented | `npm run check:release-metadata` verifies source versions, product identity, CLI binary naming, and release-doc artifact names before packaging. |
| CLI and renderer version visibility | Implemented | `c4lens --version` and the desktop status bar expose the same release version checked by `npm run check:release-metadata`. |
| macOS release manifest | Implemented | `npm run package:verify` writes and validates `release-manifest.json` with artifact paths, DMG size, and DMG SHA-256. |
| MVP demo repository fixture | Implemented | `npm run demo:mvp-repo` creates the mixed-language repo fixture used by manual smoke and `npm run smoke:mvp`. |
| MVP documentation contract check | Implemented | `npm run check:mvp-docs` keeps release-facing docs aligned with the MVP export and quality-gate contract. |
| CLI quickstart | Implemented | `docs/cli-quickstart.md` documents the first-run CLI path from `init` and `doctor` through scan, generation, drift check, and validation. |
| MVP first-run walkthrough | Implemented | `docs/mvp-first-run-walkthrough.md` gives reviewers a concise CLI plus desktop validation path against the demo repository. |
| MVP manual QA template | Implemented | `docs/mvp-manual-qa-template.md` captures candidate metadata, automated gate results, and manual release smoke outcomes. |
| First-run CLI QA gate | Implemented | `npm run qa:first-run` creates a demo repo with isolated state and verifies schema refresh, doctor readiness, scan, generation, write, drift check, and validation. |
| Installed macOS artifact QA gate | Implemented | `npm run qa:installed-macos` mounts the DMG, copies the app to a temporary install directory, verifies installed bundle metadata, universal executable architectures, and manifest checksum. |
| MVP release-candidate QA aggregate | Implemented | `npm run qa:release-candidate` runs the first-run CLI QA, installed macOS artifact QA, and MVP docs contract before the human installed-app pass. |
| CI artifact contract QA | Implemented | `npm run qa:ci-artifact -- <workflow-run-id> <commit-sha>` verifies the expected versioned macOS CI artifact exists, is non-empty, and has not expired. |
| Installed GUI QA handoff log | Implemented | `docs/qa/mvp-installed-gui-2026-07-03.md` records the current candidate artifact and the remaining human installed-app interaction checklist. |
| Current commit CI artifact QA | Implemented | `npm run qa:current-ci-artifact -- <commit-sha>` locates the successful CI run for a pushed commit and verifies the matching macOS artifact. |
| Installed GUI handoff generator | Implemented | `npm run qa:gui-handoff -- <workflow-run-id> <commit-sha>` writes a dated human GUI QA handoff from CI artifact metadata. |
| CI candidate preparation | Implemented | `npm run qa:prepare-ci-candidate -- <workflow-run-id> <commit-sha>` downloads the verified CI artifact and prepares a locally checked bundle for installed-app QA. |
| Current manual QA result stub | Implemented | `docs/qa/mvp-manual-qa-6ad137f-2026-07-03.md` is prefilled with current candidate metadata and the remaining installed-app GUI checks. |
| Manual QA stub generator | Implemented | `npm run qa:manual-stub -- <workflow-run-id> <commit-sha>` writes a dated manual QA result stub from CI artifact metadata. |
| Candidate packet check | Implemented | `npm run qa:candidate-packet -- <workflow-run-id> <commit-sha>` verifies the artifact log, GUI handoff, manual QA stub, and prepared bundle agree. |
| CI artifact retention and versioning | Implemented | CI uploads macOS artifacts as `c4lens-<version>-macos-universal-<commit-sha>` with 14-day retention. |
| Release artifact handling guide | Implemented | `docs/release-artifact-handling.md` documents CI artifact selection, manifest verification, and retention expectations. |
| MVP QA triage | Implemented | `docs/mvp-qa-triage.md` defines blocker/high/medium/low handling before an internal candidate is shared. |
| Signing/notarization decision | Implemented | `docs/signing-notarization.md` records that the MVP remains unsigned for internal validation and defines the follow-up gate. |
| Internal MVP release notes | Implemented | `docs/mvp-release-notes.md` summarizes shipped scope, verification, artifact names, known limits, and upgrade notes for the internal candidate. |
| Internal MVP release checklist | Implemented | `docs/mvp-release-checklist.md` captures the automated gate, artifact paths, manual smoke, and known non-blocking MVP limits. |

## Later Backlog

| Item | Status | Notes |
| --- | --- | --- |
| Rendered L4 code-level views | Deferred | Use indexed symbols to render lower-level diagrams once MVP navigation and generation are stable. |
| LSP-backed relationship inference | Deferred | Improve relationship accuracy beyond manifest and import heuristics. |
| Generated slug rename/move preservation | Deferred | Preserve generated slugs across file moves instead of relying on validator-reported dangling references. |
| Multi-repo workspace | Deferred | Support more than one active repository once single-repo workflows are complete. |
| Local agent API | Deferred | Expose app data to local automation only after the desktop workflow is stable. |

## Backlog Rules

- Keep disabled controls only when they map to a roadmap item.
- Move implemented roadmap items into release notes or delete them from this
  file once they ship.
- Track code quality problems in `technical_debt.md`, not here.
