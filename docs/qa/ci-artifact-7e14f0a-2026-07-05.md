# CI Artifact Confirmation - 7e14f0a - 2026-07-05

## Run

- Workflow run: `28694068981`
- Run URL: `https://github.com/fuentesjr/c4lens/actions/runs/28694068981`
- Commit: `7e14f0a68a8b94e25087f58c8f9d7ec103f6317d`
- Status: completed
- Conclusion: success

## Jobs

| Job | Result | Notes |
| --- | --- | --- |
| Check | Pass | Full quality gate passed on the pushed candidate commit. |
| Package macOS | Pass | Unsigned universal macOS build, package verification, release version read, and artifact upload completed. |

## Artifact

- Name:
  `c4lens-0.1.0-macos-universal-7e14f0a68a8b94e25087f58c8f9d7ec103f6317d`
- Expired: false
- Expires at: `2026-07-18T03:56:31Z`
- Size: 24593645 bytes

Verified with:

```sh
npm run qa:current-ci-artifact -- 7e14f0a68a8b94e25087f58c8f9d7ec103f6317d
npm run qa:prepare-ci-candidate -- 28694068981 7e14f0a68a8b94e25087f58c8f9d7ec103f6317d
npm run qa:ready-for-human -- 28694068981 7e14f0a68a8b94e25087f58c8f9d7ec103f6317d
```

Prepared local paths:

```text
target/mvp-candidates/c4lens-0.1.0-macos-universal-7e14f0a68a8b94e25087f58c8f9d7ec103f6317d/macos/c4lens.app
target/mvp-candidates/c4lens-0.1.0-macos-universal-7e14f0a68a8b94e25087f58c8f9d7ec103f6317d/dmg/c4lens_0.1.0_universal.dmg
target/mvp-candidates/c4lens-0.1.0-macos-universal-7e14f0a68a8b94e25087f58c8f9d7ec103f6317d/release-manifest.json
```

The artifact name, size, expiration, downloaded bundle, installed-artifact QA,
and candidate-packet checks match the release artifact handling contract.
