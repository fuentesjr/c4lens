# CI Artifact Confirmation - e9ce78b - 2026-07-03

## Run

- Workflow run: `28692429869`
- Run URL: `https://github.com/fuentesjr/c4lens/actions/runs/28692429869`
- Commit: `e9ce78be93a79370e80064f94ab91bfbb9a6fff2`
- Status: completed
- Conclusion: success

## Jobs

| Job | Result | Notes |
| --- | --- | --- |
| Package macOS | Pass | Unsigned universal macOS build, package verification, release version read, and artifact upload completed. |
| Check | Pass | Full quality gate passed on the pushed candidate commit. |

## Artifact

- Name:
  `c4lens-0.1.0-macos-universal-e9ce78be93a79370e80064f94ab91bfbb9a6fff2`
- Expired: false
- Expires at: `2026-07-18T02:47:05Z`
- Size: 24603377 bytes

Verified with:

```sh
npm run qa:current-ci-artifact -- e9ce78be93a79370e80064f94ab91bfbb9a6fff2
npm run qa:prepare-ci-candidate -- 28692429869 e9ce78be93a79370e80064f94ab91bfbb9a6fff2
npm run qa:ready-for-human -- 28692429869 e9ce78be93a79370e80064f94ab91bfbb9a6fff2
```

Prepared local paths:

```text
target/mvp-candidates/c4lens-0.1.0-macos-universal-e9ce78be93a79370e80064f94ab91bfbb9a6fff2/macos/c4lens.app
target/mvp-candidates/c4lens-0.1.0-macos-universal-e9ce78be93a79370e80064f94ab91bfbb9a6fff2/dmg/c4lens_0.1.0_universal.dmg
target/mvp-candidates/c4lens-0.1.0-macos-universal-e9ce78be93a79370e80064f94ab91bfbb9a6fff2/release-manifest.json
```

The artifact name, size, expiration, downloaded bundle, installed-artifact QA,
and candidate-packet checks match the release artifact handling contract.
