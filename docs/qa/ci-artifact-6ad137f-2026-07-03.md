# CI Artifact Confirmation - 6ad137f - 2026-07-03

## Run

- Workflow run: `28688677805`
- Run URL: `https://github.com/fuentesjr/c4lens/actions/runs/28688677805`
- Commit: `6ad137f5633b045ebdd41a2c29a76da426db83c3`
- Status: completed
- Conclusion: success

## Jobs

| Job | Result | Notes |
| --- | --- | --- |
| Check | Pass | Full quality gate passed on the pushed current-candidate preparation commit. |
| Package macOS | Pass | Unsigned universal macOS build, package verification, release version read, and artifact upload completed. |

## Artifact

- Name:
  `c4lens-0.1.0-macos-universal-6ad137f5633b045ebdd41a2c29a76da426db83c3`
- Expired: false
- Expires at: `2026-07-18T00:09:19Z`
- Size: 24593604 bytes

Verified with:

```sh
npm run qa:current-ci-artifact -- 6ad137f5633b045ebdd41a2c29a76da426db83c3
npm run qa:prepare-ci-candidate -- 28688677805 6ad137f5633b045ebdd41a2c29a76da426db83c3
npm run qa:candidate-packet -- 28688677805 6ad137f5633b045ebdd41a2c29a76da426db83c3
```

Prepared local paths:

```text
target/mvp-candidates/c4lens-0.1.0-macos-universal-6ad137f5633b045ebdd41a2c29a76da426db83c3/macos/c4lens.app
target/mvp-candidates/c4lens-0.1.0-macos-universal-6ad137f5633b045ebdd41a2c29a76da426db83c3/dmg/c4lens_0.1.0_universal.dmg
target/mvp-candidates/c4lens-0.1.0-macos-universal-6ad137f5633b045ebdd41a2c29a76da426db83c3/release-manifest.json
```

The artifact name, size, expiration, downloaded bundle, installed-artifact QA,
and candidate-packet checks match the release artifact handling contract.
