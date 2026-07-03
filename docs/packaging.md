# Packaging

The MVP packaging target is macOS: a universal app bundle and DMG for arm64 and
x86_64.

## macOS Universal Build

Install both Rust macOS targets before building:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

Build unsigned internal development artifacts:

```sh
npm run tauri:build:macos
```

This builds the unsigned Tauri app bundle and then creates a simple DMG with
`hdiutil`:

```sh
tauri build --target universal-apple-darwin --bundles app --ci --no-sign
hdiutil create -srcfolder <staging-dir> -format UDZO c4lens_<version>_universal.dmg
```

Artifacts are written under `target/universal-apple-darwin/release/bundle/`.
`npm run package:verify` also writes
`target/universal-apple-darwin/release/bundle/release-manifest.json`.

Verify that the app bundle, DMG, `Info.plist`, release metadata, executable,
universal architectures, and DMG checksum are present and valid:

```sh
npm run package:verify
```

Run the full local release smoke on macOS:

```sh
npm run smoke:release
```

This runs the quality gate, MVP smoke, unsigned universal macOS build, and
artifact verification. The artifact verifier is the packaged-app smoke for the
unsigned MVP build: it checks the app bundle metadata, universal executable, DMG
image integrity, and release manifest before artifacts are uploaded. CI also
runs the unsigned macOS packaging job on pushes to `main` and manual workflow
dispatches.

CI uploads the verified bundle with a versioned artifact name:

```text
c4lens-<version>-macos-universal-<commit-sha>
```

The upload uses `retention-days: 14`; use the artifact only as a short-lived
internal candidate. Use `release-manifest.json` to confirm the packaged version,
DMG path, byte size, and SHA-256 before sharing. See
[release artifact handling](release-artifact-handling.md) for the full
selection and verification process.

Use the [MVP release checklist](mvp-release-checklist.md) and
[MVP release notes](mvp-release-notes.md) before sharing an internal macOS
candidate.

## Signing

Unsigned artifacts are for internal development only. Before sharing outside
local development, produce a signed and notarized build and remove `--no-sign`
from the release command.

The current internal MVP decision is captured in
[signing and notarization decision](signing-notarization.md).
