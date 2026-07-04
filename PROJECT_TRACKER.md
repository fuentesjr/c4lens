# Project Tracker

Last updated: 2026-07-03

This file is the working tracker for MVP progress. Use it with
`docs/roadmap.md`, `docs/mvp-release-checklist.md`, and
`docs/mvp-release-notes.md`: the docs describe product/release contracts; this
file tracks execution state across task batches.

## Current Status

- MVP feature scope: implemented.
- Current workstream: internal macOS MVP release-candidate hardening.
- Branch: `main`.
- Push status: report in the final response for each completed batch.
- Release target: unsigned universal macOS app plus DMG for internal
  validation.

## In Flight

Current batch: none.

Last completed batch:

- Pushed prior local release-candidate commit `3791a9a` upstream.
- Confirmed pushed CI run `28688315327` passed for
  `3791a9a75ab1232bab1c61741980ab1ac97ba4de` and recorded
  `docs/qa/ci-artifact-3791a9a-2026-07-03.md`.
- Added `npm run qa:prepare-ci-candidate -- <workflow-run-id> <commit-sha>` to
  download the verified CI artifact, verify the bundle, and prepare a local
  candidate directory under `target/mvp-candidates/`.
- Refreshed `docs/qa/mvp-installed-gui-2026-07-03.md` and added
  `docs/qa/mvp-manual-qa-3791a9a-2026-07-03.md` for the remaining human
  installed-app GUI pass.
- Total elapsed task time: 11m 19s, from 2026-07-03 16:50:06 PDT to
  2026-07-03 17:01:25 PDT.

Verification status:

- `gh run watch 28688315327 --repo fuentesjr/c4lens --exit-status`: passed.
- `npm run qa:release-candidate`: passed.
- `npm run qa:current-ci-artifact --
  3791a9a75ab1232bab1c61741980ab1ac97ba4de`: passed; artifact
  `c4lens-0.1.0-macos-universal-3791a9a75ab1232bab1c61741980ab1ac97ba4de`
  expires `2026-07-17T23:54:57Z` and is 24593564 bytes.
- `npm run qa:prepare-ci-candidate -- 28688315327
  3791a9a75ab1232bab1c61741980ab1ac97ba4de`: passed; prepared bundle under
  `target/mvp-candidates/c4lens-0.1.0-macos-universal-3791a9a75ab1232bab1c61741980ab1ac97ba4de`.
- `npm run qa:gui-handoff -- 28688315327
  3791a9a75ab1232bab1c61741980ab1ac97ba4de
  docs/qa/mvp-installed-gui-2026-07-03.md`: passed.
- `npm run check:all`: passed.
- `bash -n scripts/qa_current_ci_artifact.sh
  scripts/write_mvp_installed_gui_handoff.sh scripts/prepare_ci_candidate.sh`:
  passed.
- `git diff --check`: passed.
- Human installed-app GUI pass: not run; use
  `docs/qa/mvp-manual-qa-3791a9a-2026-07-03.md`.

## Recent Batches

| Commit | Batch | Result |
| --- | --- | --- |
| `3791a9a` | Current CI artifact handoff | Added current-commit CI artifact verification, GUI handoff generation, recorded `cf5b712` artifact metadata, and passed `npm run check:all`. |
| `cf5b712` | Release-candidate QA checks | Added `npm run qa:release-candidate`, CI artifact metadata verification, installed-GUI handoff logging, docs wiring, and passed `npm run check:all`. |
| `56bec41` | Installed macOS artifact QA | Added `npm run qa:installed-macos`, recorded automated installed-artifact QA, and captured CI artifact metadata for run `28686171140`. |
| `26e8e04` | CI Rust setup fix | Fixed CI Rust component setup syntax after push validation, docs contract coverage, and confirmed follow-up CI passed with artifact upload. |
| `288caf6` | MVP first-run QA gate | Added `npm run qa:first-run`, first-run QA result logging, walkthrough corrections, and passed `npm run check:all`. |
| `ba9a730` | MVP release artifact workflow hardening | Added versioned/retained CI artifacts, artifact handling docs, signing/notarization decision, QA triage, and passed `npm run check:all`. |
| `769b71e` | MVP release execution guides | Added CLI quickstart, first-run walkthrough, manual QA template, tracker updates, and passed `npm run smoke:release`. |
| `3d3376f` | CLI repo doctor and project tracker | Added `c4lens doctor`, doctor integration coverage, MVP smoke coverage, and the project tracker. |
| `516a883` | CLI onboarding commands | Added `c4lens init`, `c4lens schema`, CLI tests, MVP smoke coverage, and onboarding docs. |
| `132fbdd` | MVP release artifacts | Added CLI/app version visibility and macOS `release-manifest.json` generation/verification. |
| `f4440ac` | MVP release readiness checks | Added MVP release notes, release metadata checks, and reusable demo repo fixture. |
| `8d9d051` | MVP PDF export support | Added PDF export across renderer, IPC, backend, tests, and docs. |
| `9476b1e` | MVP release readiness hardening | Continued packaging/release readiness before the release-notes batch. |
| `87771d3` | MVP readiness workflow polish | Continued smoke/workflow hardening before release packaging. |
| `a65a720` | MVP release packaging gate | Added macOS packaging/release gate groundwork. |
| `bdc729e` | Cross-language generation | Generated relationships from cross-language internal imports. |
| `7a61245` | MVP language indexing | Expanded MVP indexing coverage for TypeScript/JavaScript, Ruby, Rust, Go, and Python. |

## Release Gates

Required before sharing an internal MVP candidate:

- `npm run check:all`
- `npm run smoke:mvp`
- `npm run smoke:release`

Useful targeted checks:

- `cargo test -p c4lens-cli --test init`
- `cargo test -p c4lens-cli --test doctor`
- `npm run check:release-metadata`
- `npm run check:mvp-docs`
- `npm run package:verify`
- `npm run qa:release-candidate`
- `npm run qa:ci-artifact -- <workflow-run-id> <commit-sha>`
- `npm run qa:current-ci-artifact -- <commit-sha>`
- `npm run qa:gui-handoff -- <workflow-run-id> <commit-sha>`
- `npm run qa:prepare-ci-candidate -- <workflow-run-id> <commit-sha>`

## Next Candidate Tasks

Pick from this list when the user asks for the next MVP task batch:

- Run the human installed-app GUI pass and fill out
  `docs/qa/mvp-manual-qa-3791a9a-2026-07-03.md`.
- Resolve blocker or high-severity findings from the installed desktop pass
  using `docs/mvp-qa-triage.md`.
- Use the prepared candidate under `target/mvp-candidates/`, or use the CI
  artifact from run `28688315327` while it is retained. Rebuild locally with
  `npm run smoke:release` after `2026-07-17T23:54:57Z`.
- Push this task-batch commit before expecting CI artifact coverage for these
  script/doc updates.
- If the candidate needs to be shared beyond internal reviewers, start the
  signed/notarized follow-up from `docs/signing-notarization.md`.

## Known Non-Blocking MVP Limits

- No rendered L4 code-level views.
- No LSP-backed relationship inference.
- No generated-slug rename or move preservation.
- No multi-repo workspace.
- No local agent API.
- No signed/notarized installer.
- No auto-updater.

## Tracker Rules

- Update this file at the start or end of every MVP task batch.
- Keep `In Flight` accurate before a commit.
- Move completed batch details into `Recent Batches` after commit.
- Keep roadmap feature status in `docs/roadmap.md`; use this file for execution
  status and next-batch selection.
