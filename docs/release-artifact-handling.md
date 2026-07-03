# Release Artifact Handling

Use this guide when selecting, verifying, and sharing an internal macOS MVP
candidate artifact.

## Local Artifacts

Run the release smoke on macOS:

```sh
npm run smoke:release
```

The local artifact bundle is written under:

```text
target/universal-apple-darwin/release/bundle/
```

Expected files:

```text
target/universal-apple-darwin/release/bundle/macos/c4lens.app
target/universal-apple-darwin/release/bundle/dmg/c4lens_0.1.0_universal.dmg
target/universal-apple-darwin/release/bundle/release-manifest.json
```

Before sharing a locally built candidate, run:

```sh
npm run package:verify
npm run qa:installed-macos
npm run qa:release-candidate
```

`qa:installed-macos` mounts the DMG, copies `c4lens.app` into a temporary
install directory, verifies the installed app bundle metadata and universal
executable, and checks the manifest DMG checksum against the artifact.

## CI Artifacts

The `Package macOS` workflow job uploads the verified bundle on pushes to
`main` and manual workflow dispatches.

Artifact naming contract:

```text
c4lens-<version>-macos-universal-<commit-sha>
```

The upload uses `retention-days: 14`. Treat the artifact as a short-lived
internal validation candidate, not durable release storage.

Verify the CI artifact metadata before downloading or sharing it:

```sh
npm run qa:ci-artifact -- <workflow-run-id> <commit-sha>
npm run qa:current-ci-artifact -- <commit-sha>
```

The command checks that the expected
`c4lens-<version>-macos-universal-<commit-sha>` artifact exists, is non-empty,
has not expired, and has an expiration timestamp.

For a pushed commit on the current branch, prefer
`npm run qa:current-ci-artifact -- <commit-sha>`; it finds the successful `CI`
run before checking the artifact metadata. Once the artifact is selected,
generate the installed-app handoff with:

```sh
npm run qa:gui-handoff -- <workflow-run-id> <commit-sha>
```

## Version And Commit Verification

Use `release-manifest.json` as the source of truth for the packaged version and
DMG checksum:

```json
{
  "version": "0.1.0",
  "platform": "macos-universal",
  "artifacts": {
    "dmg": {
      "path": "dmg/c4lens_0.1.0_universal.dmg",
      "sha256": "<sha256>"
    }
  }
}
```

Before sending an artifact to an internal reviewer:

- Match the CI artifact commit SHA to the intended commit.
- Match `release-manifest.json` version to the candidate version.
- Match the DMG filename to the manifest version.
- Record the artifact name, workflow run, commit, version, and DMG SHA-256 in
  the [MVP manual QA template](mvp-manual-qa-template.md).

## Retention

The GitHub Actions upload expires after 14 days. If a candidate needs to remain
available after that window, rebuild it from the recorded commit or promote it
into a proper release channel after signing and notarization are in place.
