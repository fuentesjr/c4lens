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

- Pushed prior local ready-for-human packet commit `dcba6e2` upstream.
- Used agenticons (`planner`, `helper_worker`, `doc_reviewer`, and `reviewer`)
  to select the next release tasks, audit stale current-candidate references,
  and review the final diff.
- Confirmed pushed CI run `28691766065` passed for
  `dcba6e204cfd4450b6fd1753ee368ec34ddc58d9` and recorded
  `docs/qa/ci-artifact-dcba6e2-2026-07-03.md`.
- Artifact
  `c4lens-0.1.0-macos-universal-dcba6e204cfd4450b6fd1753ee368ec34ddc58d9`
  expires `2026-07-18T02:18:17Z` and is 24593814 bytes.
- Refreshed the current-head ready-for-human packet for `dcba6e2` after the
  upstream CI artifact was uploaded.
- Refreshed `docs/qa/mvp-installed-gui-2026-07-03.md` and generated
  `docs/qa/mvp-manual-qa-dcba6e2-2026-07-03.md` for the current candidate.
- Updated the release checklist, roadmap, and MVP docs contract checks so the
  live current-candidate references point at `dcba6e2` while older packet docs
  remain historical records. Tightened the current manual QA check to require
  the human GUI pass checkbox to remain unchecked.
- Prepared local candidate paths under
  `target/mvp-candidates/c4lens-0.1.0-macos-universal-dcba6e204cfd4450b6fd1753ee368ec34ddc58d9/`.
- Human installed-app GUI pass remains the only unrun candidate gate; record it
  in `docs/qa/mvp-manual-qa-dcba6e2-2026-07-03.md`. Manual-finding triage is
  not started because the human GUI pass has not produced findings yet.
- Total elapsed task time: 15m 45s, from 2026-07-03 19:10:59 PDT to
  2026-07-03 19:26:44 PDT.

Verification status:

- `git push`: passed; pushed `dcba6e2` to `origin/main`.
- `gh run view 28691766065 --repo fuentesjr/c4lens --json status,conclusion,jobs,url`:
  passed; `Check` and `Package macOS` completed successfully.
- `npm run qa:ready-for-human -- 28691766065
  dcba6e204cfd4450b6fd1753ee368ec34ddc58d9 2026-07-03`: passed; generated and checked the
  full human QA packet.
- `npm run qa:release-candidate`: passed.
- `npm run qa:current-ci-artifact --
  dcba6e204cfd4450b6fd1753ee368ec34ddc58d9`: passed through the
  ready-for-human command; artifact
  `c4lens-0.1.0-macos-universal-dcba6e204cfd4450b6fd1753ee368ec34ddc58d9`
  expires `2026-07-18T02:18:17Z` and is 24593814 bytes.
- `npm run qa:artifact-log -- 28691766065
  dcba6e204cfd4450b6fd1753ee368ec34ddc58d9`: passed through the
  ready-for-human command; recorded
  `docs/qa/ci-artifact-dcba6e2-2026-07-03.md`.
- `npm run qa:prepare-ci-candidate -- 28691766065
  dcba6e204cfd4450b6fd1753ee368ec34ddc58d9`: passed; prepared bundle under
  `target/mvp-candidates/c4lens-0.1.0-macos-universal-dcba6e204cfd4450b6fd1753ee368ec34ddc58d9`.
- `npm run qa:gui-handoff -- 28691766065
  dcba6e204cfd4450b6fd1753ee368ec34ddc58d9
  docs/qa/mvp-installed-gui-2026-07-03.md`: passed.
- `npm run qa:manual-stub -- 28691766065
  dcba6e204cfd4450b6fd1753ee368ec34ddc58d9`: passed through the
  ready-for-human command.
- `npm run qa:candidate-packet -- 28691766065
  dcba6e204cfd4450b6fd1753ee368ec34ddc58d9 2026-07-03`: passed through the
  ready-for-human command.
- `npm run check:mvp-docs`: passed.
- `npm run check:all`: passed.
- `bash -n scripts/check_mvp_docs.sh`: passed.
- `git diff --check`: passed.
- Human installed-app GUI pass: not run; use
  `docs/qa/mvp-manual-qa-dcba6e2-2026-07-03.md`.

## Recent Batches

| Commit | Batch | Result |
| --- | --- | --- |
| `dcba6e2` | Current-head ready-for-human packet refresh | Pushed the packet automation commit, confirmed CI run `28691766065`, regenerated the current packet for `dcba6e2`, and left only the human installed-app GUI pass. |
| `32b2f34` | Ready-for-human candidate packet | Added artifact-log generation, one-command human QA packet preparation, regenerated the current packet for run `28689213998`, and left only the human installed-app GUI pass. |
| `6ad137f` | Current MVP CI candidate preparation | Added CI candidate preparation, recorded `3791a9a` artifact metadata, prepared the current candidate locally, and passed `npm run check:all`. |
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
- `npm run qa:artifact-log -- <workflow-run-id> <commit-sha>`
- `npm run qa:gui-handoff -- <workflow-run-id> <commit-sha>`
- `npm run qa:prepare-ci-candidate -- <workflow-run-id> <commit-sha>`
- `npm run qa:manual-stub -- <workflow-run-id> <commit-sha>`
- `npm run qa:candidate-packet -- <workflow-run-id> <commit-sha>`
- `npm run qa:ready-for-human -- <workflow-run-id> <commit-sha>`

## Next Candidate Tasks

Pick from this list when the user asks for the next MVP task batch:

- Run the human installed-app GUI pass and fill out
  `docs/qa/mvp-manual-qa-dcba6e2-2026-07-03.md`.
- Resolve blocker or high-severity findings from the installed desktop pass
  using `docs/mvp-qa-triage.md`.
- Use the prepared candidate under `target/mvp-candidates/`, or use the CI
  artifact from run `28691766065` while it is retained. Rebuild locally with
  `npm run smoke:release` after `2026-07-18T02:18:17Z`.
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
