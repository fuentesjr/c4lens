# MVP Release Checklist

Use this checklist for an internal macOS MVP candidate. The MVP artifact is an
unsigned universal macOS app bundle plus DMG. Signing, notarization, updater
support, Windows, and Linux are outside this internal release gate.

## Preconditions

- Worktree is clean before starting the release smoke.
- Current branch contains the intended release commits.
- JavaScript dependencies are installed with `npm ci` or `npm install`.
- Rust 1.96.0 is available. When `mise` is installed, project scripts use
  `mise exec rust@1.96.0 -- ...`.
- The command is run on macOS.

## Automated Gate

Run the full local release gate:

```sh
npm run smoke:release
```

This must pass before sharing an internal MVP artifact. It runs:

- `npm run check:all`
- `npm run smoke:mvp`
- `npm run tauri:build:macos`
- `npm run package:verify`

The quality gate includes:

- Rust format, Clippy, and test suite.
- Release metadata contract check.
- Renderer typecheck and test suite.
- Tauri capability and production CSP checks.
- MVP documentation contract check.
- Git whitespace checks.

The MVP smoke covers:

- CLI init, schema refresh, and doctor checks for first-run repository setup.
- CLI validate, scan, generate preview, generate write, and drift check.
- Multi-language symbol/import indexing and generated import relationships.
- Writer contention for scan and generated overlay writes.
- Renderer workflow E2E tests, including search, validation, generation,
  jump-to-code, SVG export, PDF export, and PNG export.

The package verifier checks:

- `c4lens.app` exists.
- DMG exists and is non-empty.
- `release-manifest.json` is written.
- `Info.plist` is valid.
- Bundle name, identifier, and version match `tauri.conf.json`.
- App executable exists and is universal for `x86_64` and `arm64`.
- DMG checksum verifies with `hdiutil` and matches the release manifest.
- The installed-artifact QA gate can mount the DMG, copy `c4lens.app` to a
  temporary install directory, and verify installed bundle metadata.

For a focused release-candidate pass before human GUI validation, run:

```sh
npm run qa:release-candidate
```

This runs the first-run CLI QA, installed macOS artifact QA, and MVP
documentation contract check.

## Artifact Location

After a passing release smoke, artifacts are under:

```text
target/universal-apple-darwin/release/bundle/
```

Expected internal artifacts:

```text
target/universal-apple-darwin/release/bundle/macos/c4lens.app
target/universal-apple-darwin/release/bundle/dmg/c4lens_0.1.0_universal.dmg
target/universal-apple-darwin/release/bundle/release-manifest.json
```

CI also uploads this bundle directory from the macOS packaging job on pushes to
`main` and manual workflow dispatches. CI artifact names follow
`c4lens-<version>-macos-universal-<commit-sha>` and are retained for 14 days.
Use [release artifact handling](release-artifact-handling.md) before sharing a
candidate from CI.

To verify the current CI artifact metadata before installing it, run:

```sh
npm run qa:ci-artifact -- <workflow-run-id> <commit-sha>
npm run qa:current-ci-artifact -- <commit-sha>
npm run qa:prepare-ci-candidate -- <workflow-run-id> <commit-sha>
```

Use [MVP release notes](mvp-release-notes.md) as the candidate summary when
sharing an internal build.

## Manual Smoke

Use the [MVP first-run walkthrough](mvp-first-run-walkthrough.md) for a concise
end-to-end validation path, and record formal candidate results with the
[MVP manual QA template](mvp-manual-qa-template.md). Classify any findings with
[MVP QA triage](mvp-qa-triage.md).

For the current candidate handoff, use
`docs/qa/mvp-installed-gui-2026-07-03.md` to drive and record the remaining
installed-app interaction pass.

For the current candidate result record, use
`docs/qa/mvp-manual-qa-6ad137f-2026-07-03.md`.

To regenerate the handoff from CI artifact metadata for a newer candidate, run:

```sh
npm run qa:gui-handoff -- <workflow-run-id> <commit-sha>
npm run qa:manual-stub -- <workflow-run-id> <commit-sha>
npm run qa:candidate-packet -- <workflow-run-id> <commit-sha>
```

To create a disposable repository for manual MVP checks:

```sh
npm run demo:mvp-repo -- /tmp/c4lens-mvp-demo
```

Before calling an internal candidate ready, install from the DMG on a current
supported macOS machine and exercise these workflows:

- Run `npm run qa:installed-macos` against the local release bundle.
- Launch the app from the installed `c4lens.app`.
- Confirm the status bar shows the expected app version.
- Open a local repository.
- In a disposable repository, run `c4lens init --repo <repo> --name "Manual Smoke"`
  and confirm `c4/model.yml` plus `c4/schema.json` are created.
- Run `c4lens schema --repo <repo>` after editing `c4/schema.json` and confirm
  the bundled schema is restored.
- Run `c4lens doctor --repo <repo>` and confirm it reports the repo as ready.
- Validate a valid `c4/model.yml`.
- Introduce an invalid model edit and confirm the last valid canvas remains
  visible while validation errors show path/line/column details.
- Run Scan and confirm status counts update.
- Run Generate, review the diff, apply the generated overlay, and confirm
  `c4/model.generated.yml` plus `c4/schema.json` are written.
- Select a generated element and confirm provenance is visible.
- Use global search with keyboard navigation to open an element and a file.
- Jump to code from the detail panel.
- Export SVG, PDF, and PNG from the current view.
- Toggle light and dark themes.
- Resize to the configured minimum window size and confirm the canvas and detail
  panel remain usable.
- Run `target/debug/c4lens --version` from a local build and confirm it matches
  the app version.

## Known MVP Limits

These are intentionally not blockers for the internal MVP candidate:

- No rendered L4 code-level views.
- No LSP-backed relationship inference.
- No generated-slug rename or move preservation.
- No multi-repo workspace.
- No local agent API.
- No signed/notarized installer.
- No auto-updater.

The unsigned internal-release decision and follow-up gate are documented in
[signing and notarization decision](signing-notarization.md).
