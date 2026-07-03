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
- Renderer typecheck and test suite.
- Tauri capability and production CSP checks.
- MVP documentation contract check.
- Git whitespace checks.

The MVP smoke covers:

- CLI validate, scan, generate preview, generate write, and drift check.
- Multi-language symbol/import indexing and generated import relationships.
- Writer contention for scan and generated overlay writes.
- Renderer workflow E2E tests, including search, validation, generation,
  jump-to-code, SVG export, PDF export, and PNG export.

The package verifier checks:

- `c4lens.app` exists.
- DMG exists and is non-empty.
- `Info.plist` is valid.
- Bundle name, identifier, and version match `tauri.conf.json`.
- App executable exists and is universal for `x86_64` and `arm64`.
- DMG checksum verifies with `hdiutil`.

## Artifact Location

After a passing release smoke, artifacts are under:

```text
target/universal-apple-darwin/release/bundle/
```

Expected internal artifacts:

```text
target/universal-apple-darwin/release/bundle/macos/c4lens.app
target/universal-apple-darwin/release/bundle/dmg/c4lens_0.1.0_universal.dmg
```

CI also uploads this bundle directory from the macOS packaging job on pushes to
`main` and manual workflow dispatches.

## Manual Smoke

Before calling an internal candidate ready, install from the DMG on a current
supported macOS machine and exercise these workflows:

- Launch the app from the installed `c4lens.app`.
- Open a local repository.
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

## Known MVP Limits

These are intentionally not blockers for the internal MVP candidate:

- No rendered L4 code-level views.
- No LSP-backed relationship inference.
- No generated-slug rename or move preservation.
- No multi-repo workspace.
- No local agent API.
- No signed/notarized installer.
- No auto-updater.
