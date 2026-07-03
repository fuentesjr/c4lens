# MVP Installed Artifact QA Result - 2026-07-03

## Candidate

- Candidate version: 0.1.0
- Candidate branch: `main`
- Candidate commit: `26e8e04`
- Artifact source: local macOS release bundle.
- Artifact path:
  `target/universal-apple-darwin/release/bundle/dmg/c4lens_0.1.0_universal.dmg`

## Commands

```sh
npm run qa:installed-macos
```

## Results

| Area | Result | Notes |
| --- | --- | --- |
| DMG integrity | Pass | `npm run package:verify` verified the DMG checksum with `hdiutil`. |
| DMG mount | Skipped | This local environment reported `hdiutil: attach failed - Device not configured` for the APFS DMG. |
| App install simulation | Pass | `c4lens.app` copied from the verified packaged app fallback to a temporary install directory. The QA script uses the mounted DMG app when attach is available. |
| Installed bundle metadata | Pass | Installed `Info.plist` matched product name, identifier, and version. |
| Installed executable | Pass | Installed app executable was present, executable, and universal for `x86_64` and `arm64`. |
| Manifest checksum | Pass | `release-manifest.json` DMG SHA-256 matched the artifact. |
| Blocker/high findings | None open | No blocker or high-severity findings were found in the automated installed-artifact pass. |

## Scope Note

This verifies the installable artifact structure and metadata without launching
the GUI. The remaining human QA step is the installed desktop interaction pass
from [MVP first-run walkthrough](../mvp-first-run-walkthrough.md).
