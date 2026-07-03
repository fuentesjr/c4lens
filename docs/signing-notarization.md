# Signing And Notarization Decision

Decision: the internal macOS MVP candidate remains unsigned and unnotarized.

This is acceptable only for local/internal validation. A signed and notarized
build is required before wider sharing, public distribution, or auto-update
support.

## MVP Scope

In scope for the internal MVP:

- Unsigned universal macOS app bundle.
- Unsigned DMG.
- `npm run smoke:release` as the release gate.
- Manual reviewer instructions that call out the unsigned candidate status.

Out of scope for the internal MVP:

- Developer ID Application signing.
- Apple notarization and stapling.
- Signed update manifests.
- Auto-updater support.

## Follow-Up Gate

Before sharing beyond internal reviewers, add a signing/notarization release
gate that proves:

- Apple Developer ID credentials are available in CI or the release machine.
- Tauri signing configuration no longer uses `--no-sign`.
- The app bundle and DMG are signed.
- The DMG is notarized and stapled.
- `spctl --assess` accepts the installed app.
- `npm run smoke:release` or its successor verifies signed artifact metadata.

## Release Notes Language

Keep internal candidate notes explicit:

```text
This build is unsigned and unnotarized. It is for internal validation only.
```

Remove that language only after the signing/notarization follow-up gate passes
for the candidate being shared.
