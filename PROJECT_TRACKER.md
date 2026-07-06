# Project Tracker

Last updated: 2026-07-05

This file is the working tracker for execution state across task batches. Use
it with `docs/roadmap.md`, `docs/mvp-release-checklist.md`, and
`docs/mvp-release-notes.md`: the docs describe product/release contracts; this
file tracks execution state and next-batch selection.

## Current Status

- MVP feature scope: implemented.
- Current workstream: generation quality (code→model inference), validated
  against real-shaped repositories and this repository's own `c4/model.yml`.
- Release-process workstream: frozen as of 2026-07-05 (see Course Correction).
- Branch: `main`.
- Push status: report in the final response for each completed batch.
- Blocking release gate: human installed-app GUI pass on the `7e14f0a`
  candidate (unrun since 2026-07-03).

## Course Correction (2026-07-05)

A whole-project review found the release-QA automation had become a
self-feeding loop (every packet-refresh commit invalidated its own packet)
while the product differentiator — code→model generation — remained the
thinnest part of the product and had never been exercised on a real-shaped or
this repository. Decisions:

1. **Freeze release-process work.** The QA packet tooling is complete. Do not
   add QA automation, and do not refresh candidate packets for commits that
   only change docs, scripts, or generation internals. The `7e14f0a` candidate
   stays the human-QA target even though `main` has moved past it.
2. **Run the human GUI pass** against the existing prepared candidate instead
   of regenerating packets first.
3. **Redirect effort to generation quality.** Track items in the roadmap's
   "Generation Quality" section; select batches from there.
4. **Dogfood.** The repo now carries its own `c4/model.yml` and generated
   overlay; regressions in self-generation are findings, not noise.

## In Flight

Current batch: none.

Last completed batch (generation quality 1, 2026-07-05):

- Added datastore/external-system dependency detection for Gemfile,
  Cargo.toml, go.mod, pyproject.toml, and requirements.txt (previously
  package.json only), sharing the existing store/external generation path.
- Compose service detection now accepts `docker-compose.yml`,
  `docker-compose.yaml`, `compose.yml`, and `compose.yaml`.
- Added a Rails-shaped realism regression: an autoloaded (require-free) Rails
  repo must generate stores, compose services, and dependency relationships
  instead of an edgeless model (`generate_scan_on_autoloaded_rails_shaped_repo_is_not_edgeless`).
- Dogfooded c4lens on itself: authored `c4/model.yml` (core, cli, desktop
  shell, renderer, symbol index store) plus generated overlay; `c4lens doctor`
  reports ready.
- Dogfood findings recorded as roadmap items: workspace members are not
  detected as containers (root workspace manifest yields one component-less
  `code: .` container), and same-named containers get numeric collision slugs
  (`c4lens_2`, `c4lens_3`).

Verification status:

- `cargo test -p c4lens-core generation`: passed (17 tests, including the new
  per-ecosystem dependency-target and compose-extension tests).
- `cargo test -p c4lens-cli --test generate`: new Rails-shaped realism test
  passed.
- `npm run check:all`: passed after `cargo fmt`.
- `c4lens init/generate --scan --write/validate/doctor` on this repository:
  passed; `doctor` reports ready.
- Human installed-app GUI pass: not run; use
  `docs/qa/mvp-manual-qa-7e14f0a-2026-07-05.md`.

## Recent Batches

| Commit | Batch | Result |
| --- | --- | --- |
| (pending) | Course correction + generation quality 1 | Froze release-process automation, added multi-ecosystem datastore detection, compose `.yaml` support, Rails-shaped realism gate, and self-model dogfood. |
| `7e14f0a` | Current-head ready-for-human packet refresh | Pushed the candidate-packet commit, confirmed CI run `28694068981`, regenerated the current packet for `7e14f0a`, and left only the human installed-app GUI pass. |
| `243b8b1` | Current-head ready-for-human packet refresh | Pushed the current-packet commit, confirmed CI run `28693651191`, regenerated the current packet for `243b8b1`, hardened the unchecked and unfilled human GUI gate, and left only the human installed-app GUI pass. |
| `e9ce78b` | Current-head ready-for-human packet refresh | Pushed the current-packet commit, confirmed CI run `28692429869`, regenerated the current packet for `e9ce78b`, and left only the human installed-app GUI pass. |
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

## Release Gates

Required before sharing an internal MVP candidate:

- `npm run check:all`
- `npm run smoke:mvp`
- `npm run smoke:release`

Useful targeted checks:

- `cargo test -p c4lens-core generation`
- `cargo test -p c4lens-cli --test generate`
- `npm run check:release-metadata`
- `npm run check:mvp-docs`
- `npm run package:verify`
- `npm run qa:release-candidate`

The full `qa:*` packet command set remains available (see README) but is
frozen: use it only when a new candidate build is actually being prepared for
human QA, never to refresh packets for doc/script-only commits.

## Next Candidate Tasks

Pick from this list when the user asks for the next task batch:

1. **Human installed-app GUI pass** (blocking, human-only): use the prepared
   candidate under `target/mvp-candidates/` or the CI artifact from run
   `28694068981` (retained until `2026-07-18T03:56:31Z`); record results in
   `docs/qa/mvp-manual-qa-7e14f0a-2026-07-05.md` and triage findings via
   `docs/mvp-qa-triage.md`. Do not refresh the packet first.
2. **Rails constant-reference relationship inference**: port the
   `tmp/generation-spike` heuristics into `c4lens-core` — record cross-file
   constant references (and ActiveRecord associations) as internal import
   edges so autoloaded Ruby repos generate real component relationships.
3. **Workspace member containers** (dogfood finding): detect Cargo workspace
   members and package.json workspaces as containers so c4lens's own model
   shows core/cli/tauri instead of one component-less `code: .` container.
4. **Generated container naming on collisions** (dogfood finding): replace
   numeric collision slugs (`c4lens_2`, `c4lens_3`) with tech- or
   path-qualified names.
5. Signed/notarized follow-up from `docs/signing-notarization.md` only when a
   candidate needs to be shared beyond internal reviewers.

## Known Non-Blocking MVP Limits

- No rendered L4 code-level views.
- No LSP-backed relationship inference.
- No generated-slug rename or move preservation.
- No multi-repo workspace.
- No local agent API.
- No signed/notarized installer.
- No auto-updater.

## Tracker Rules

- Update this file at the start or end of every task batch.
- Keep `In Flight` accurate before a commit.
- Move completed batch details into `Recent Batches` after commit.
- Keep roadmap feature status in `docs/roadmap.md`; use this file for
  execution status and next-batch selection.
- Do not refresh candidate packets or add QA automation while the
  release-process freeze holds; packet work resumes only when a new candidate
  build is prepared for human QA.
