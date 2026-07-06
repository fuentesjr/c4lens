# MVP Manual QA - 7e14f0a - 2026-07-05

Use this file to record the remaining human installed-app GUI pass for the
current internal macOS MVP candidate.

## Candidate

- Tester:
- Date:
- Machine:
- macOS version:
- Candidate version: `0.1.0`
- Candidate commit: `7e14f0a68a8b94e25087f58c8f9d7ec103f6317d`
- Artifact source: GitHub Actions workflow run `28694068981`
- Artifact name:
  `c4lens-0.1.0-macos-universal-7e14f0a68a8b94e25087f58c8f9d7ec103f6317d`
- Workflow run: `28694068981`
- App path:
  `target/mvp-candidates/c4lens-0.1.0-macos-universal-7e14f0a68a8b94e25087f58c8f9d7ec103f6317d/macos/c4lens.app`
- DMG path:
  `target/mvp-candidates/c4lens-0.1.0-macos-universal-7e14f0a68a8b94e25087f58c8f9d7ec103f6317d/dmg/c4lens_0.1.0_universal.dmg`
- `release-manifest.json` path:
  `target/mvp-candidates/c4lens-0.1.0-macos-universal-7e14f0a68a8b94e25087f58c8f9d7ec103f6317d/release-manifest.json`

## Automated Gate

- [x] CI run `28694068981` completed successfully.
- [x] `npm run qa:current-ci-artifact -- 7e14f0a68a8b94e25087f58c8f9d7ec103f6317d` passed.
- [x] `npm run qa:prepare-ci-candidate -- 28694068981 7e14f0a68a8b94e25087f58c8f9d7ec103f6317d` passed.
- [x] `npm run qa:candidate-packet -- 28694068981 7e14f0a68a8b94e25087f58c8f9d7ec103f6317d` passed.
- [ ] Human installed-app GUI pass completed.

Notes:

```text
The candidate is downloaded and verified under target/mvp-candidates/.
The remaining gate requires Finder/app interaction from an installed candidate.
```

## Manual Results

| Area | Result | Notes |
| --- | --- | --- |
| Install from DMG | Not run | Requires human GUI session. |
| Launch installed `c4lens.app` | Not run | Requires human GUI session. |
| Status bar shows expected version | Not run | Expected `0.1.0`. |
| Open local repository | Not run | Use `/tmp/c4lens-mvp-demo` or equivalent. |
| `c4lens init` creates `c4/model.yml` and `c4/schema.json` | Not run | |
| `c4lens schema` restores bundled editor schema | Not run | |
| `c4lens doctor` reports repository readiness | Not run | |
| Validate valid model | Not run | |
| Invalid model keeps last valid canvas and shows path/line/column details | Not run | |
| Scan updates source counts | Not run | |
| Generate review/apply writes `c4/model.generated.yml` | Not run | |
| Generated provenance is visible | Not run | |
| Search opens elements, files, and symbols | Not run | |
| Jump to code opens source location | Not run | |
| Export SVG/PDF/PNG succeeds | Not run | |
| Light and dark themes render correctly | Not run | |
| Minimum window size remains usable | Not run | |
| `c4lens --version` matches app version | Not run | |

## Blockers

List any issue that should prevent sharing the candidate:

```text
None recorded yet. Human installed-app GUI pass has not been run.
```

Classify findings with [MVP QA triage](../mvp-qa-triage.md).

## Follow-Ups

List non-blocking issues or release-note clarifications:

```text
None recorded yet.
```
