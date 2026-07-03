# c4lens MVP Release Notes

Version: 0.1.0

These notes describe the internal macOS MVP candidate. The artifact is an
unsigned universal macOS app bundle plus DMG for local/internal validation:

```text
target/universal-apple-darwin/release/bundle/macos/c4lens.app
target/universal-apple-darwin/release/bundle/dmg/c4lens_0.1.0_universal.dmg
```

## Shipped Scope

- Local-first C4 model navigation from `c4/model.yml`.
- Generated, disposable model overlay support through `c4/model.generated.yml`.
- Model validation with path, line, and column details where available.
- Code scanning, symbol indexing, and best-effort import extraction for
  TypeScript/JavaScript, Ruby, Rust, Go, and Python.
- Code-derived model generation with a review/apply flow.
- Drill-down C4 views, ELK layout caching, dependency highlighting, focus mode,
  provenance badges, detail panel, and jump-to-code.
- Keyboardable global search across elements, indexed files, and symbols.
- Native desktop repository open, save dialogs, source watching, and persisted
  last-repository/theme state.
- SVG, PDF, and PNG export from the current view.
- Unsigned universal macOS app and DMG packaging for internal validation.

## Verification

Run the release smoke before sharing an internal candidate:

```sh
npm run smoke:release
```

For a faster local readiness check during development:

```sh
npm run check:all
npm run smoke:mvp
```

The release gate includes Rust format, Clippy, Rust tests, renderer typecheck,
renderer tests, Tauri security checks, release metadata checks, MVP docs checks,
MVP smoke, macOS packaging, and artifact verification.

## Known Limits

- No rendered L4 code-level views.
- No LSP-backed relationship inference.
- No generated-slug rename or move preservation.
- No multi-repo workspace.
- No local agent API.
- No signed/notarized installer.
- No auto-updater.

## Upgrade Notes

This is the first internal MVP candidate. There is no migration path from a
previous release. Repositories keep authored model data in `c4/model.yml`; the
generated overlay remains disposable and can be regenerated.
