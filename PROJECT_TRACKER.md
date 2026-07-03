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

- Pushed prior local release-candidate commits through `cf5b712` upstream.
- Added `npm run qa:current-ci-artifact -- <commit-sha>` to locate the
  successful CI run for a pushed commit and verify its macOS artifact contract.
- Added `npm run qa:gui-handoff -- <workflow-run-id> <commit-sha>` to generate
  the installed-app GUI QA handoff from CI artifact metadata.
- Confirmed pushed CI run `28687518031` passed for
  `cf5b712d61b1aec4539066b258ab5cbddd525ffd` and recorded
  `docs/qa/ci-artifact-cf5b712-2026-07-03.md`.
- Regenerated `docs/qa/mvp-installed-gui-2026-07-03.md` for the current
  pushed artifact.
- Total elapsed task time: 20m 02s, from 2026-07-03 16:19:56 PDT to
  2026-07-03 16:39:58 PDT.

Verification status:

- `npm run qa:release-candidate`: passed.
- `npm run qa:current-ci-artifact --
  cf5b712d61b1aec4539066b258ab5cbddd525ffd`: passed; artifact
  `c4lens-0.1.0-macos-universal-cf5b712d61b1aec4539066b258ab5cbddd525ffd`
  expires `2026-07-17T23:24:31Z` and is 24593691 bytes.
- `npm run qa:gui-handoff -- 28687518031
  cf5b712d61b1aec4539066b258ab5cbddd525ffd
  docs/qa/mvp-installed-gui-2026-07-03.md`: passed.
- `npm run check:all`: passed.
- `bash -n scripts/qa_current_ci_artifact.sh
  scripts/write_mvp_installed_gui_handoff.sh`: passed.
- `git diff --check`: passed.
- Human installed-app GUI pass: not run; use
  `docs/qa/mvp-installed-gui-2026-07-03.md`.

## Recent Batches

| Commit | Batch | Result |
| --- | --- | --- |
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

## Next Candidate Tasks

Pick from this list when the user asks for the next MVP task batch:

- Run the human installed-app GUI pass from
  `docs/qa/mvp-installed-gui-2026-07-03.md`.
- Resolve blocker or high-severity findings from the installed desktop pass
  using `docs/mvp-qa-triage.md`.
- Use the CI artifact from run `28687518031` while it is retained, or rebuild
  locally with `npm run smoke:release` after `2026-07-17T23:24:31Z`.
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
