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

This runs Tauri with:

```sh
tauri build --target universal-apple-darwin --bundles app,dmg --ci --no-sign
```

Artifacts are written under `crates/c4lens-tauri/target/universal-apple-darwin/release/bundle/`.

## Signing

Unsigned artifacts are for internal development only. Before sharing outside
local development, produce a signed and notarized build and remove `--no-sign`
from the release command.
