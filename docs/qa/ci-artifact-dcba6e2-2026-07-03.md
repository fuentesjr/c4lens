# CI Artifact Confirmation - dcba6e2 - 2026-07-03

## Run

- Workflow run: `28691766065`
- Run URL: `https://github.com/fuentesjr/c4lens/actions/runs/28691766065`
- Commit: `dcba6e204cfd4450b6fd1753ee368ec34ddc58d9`
- Status: completed
- Conclusion: success

## Jobs

| Job | Result | Notes |
| --- | --- | --- |
| Check | Pass | Full quality gate passed on the pushed candidate commit. |
| Package macOS | Pass | Unsigned universal macOS build, package verification, release version read, and artifact upload completed. |

## Artifact

- Name:
  `c4lens-0.1.0-macos-universal-dcba6e204cfd4450b6fd1753ee368ec34ddc58d9`
- Expired: false
- Expires at: `2026-07-18T02:18:17Z`
- Size: 24593814 bytes

Verified with:

```sh
npm run qa:current-ci-artifact -- dcba6e204cfd4450b6fd1753ee368ec34ddc58d9
npm run qa:prepare-ci-candidate -- 28691766065 dcba6e204cfd4450b6fd1753ee368ec34ddc58d9
npm run qa:ready-for-human -- 28691766065 dcba6e204cfd4450b6fd1753ee368ec34ddc58d9
```

Prepared local paths:

```text
target/mvp-candidates/c4lens-0.1.0-macos-universal-dcba6e204cfd4450b6fd1753ee368ec34ddc58d9/macos/c4lens.app
target/mvp-candidates/c4lens-0.1.0-macos-universal-dcba6e204cfd4450b6fd1753ee368ec34ddc58d9/dmg/c4lens_0.1.0_universal.dmg
target/mvp-candidates/c4lens-0.1.0-macos-universal-dcba6e204cfd4450b6fd1753ee368ec34ddc58d9/release-manifest.json
```

The artifact name, size, expiration, downloaded bundle, installed-artifact QA,
and candidate-packet checks match the release artifact handling contract.
