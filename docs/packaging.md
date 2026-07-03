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
unsigned MVP build: it checks the app bundle metadata, universal executable, and
DMG image integrity before artifacts are uploaded. CI also runs the unsigned
macOS packaging job on pushes to `main` and manual workflow dispatches.

Use the [MVP release checklist](mvp-release-checklist.md) and
[MVP release notes](mvp-release-notes.md) before sharing an internal macOS
candidate.

## Signing

Unsigned artifacts are for internal development only. Before sharing outside
local development, produce a signed and notarized build and remove `--no-sign`
from the release command.
