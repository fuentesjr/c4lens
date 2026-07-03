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

- Ran the first internal first-run QA path and reproduced setup gaps in the
  walkthrough.
- Added `npm run qa:first-run` to automate the first-run CLI QA path with
  isolated c4lens state.
- Updated the first-run walkthrough to refresh schema before expecting
  `doctor` readiness.
- Recorded the first-run QA result in `docs/qa/mvp-first-run-2026-07-03.md`.
- Confirmed no signed/notarized follow-up branch is required for internal-only
  MVP validation; that work remains deferred until wider sharing.

Verification status:

- Initial ad hoc first-run CLI path: found walkthrough/setup gaps.
- `npm run smoke:mvp`: passed.
- `npm run qa:first-run`: passed.
- `npm run check:all`: passed.
- `git diff --check`: passed.

## Recent Batches

| Commit | Batch | Result |
| --- | --- | --- |
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

## Next Candidate Tasks

Pick from this list when the user asks for the next MVP task batch:

- Run the installed DMG desktop pass from
  `docs/mvp-first-run-walkthrough.md`.
- Resolve blocker or high-severity findings from the installed desktop pass
  using `docs/mvp-qa-triage.md`.
- Push the current release-candidate commits and confirm CI artifact upload
  naming/retention in GitHub Actions.
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
