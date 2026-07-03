# MVP Manual QA Template

Copy this checklist into the release issue or candidate notes for each internal
macOS MVP build.

## Candidate

- Tester:
- Date:
- Machine:
- macOS version:
- Candidate version:
- Candidate commit:
- Artifact source:
- App path:
- DMG path:
- `release-manifest.json` path:

## Automated Gate

- [ ] `npm run smoke:release` passed.
- [ ] `npm run package:verify` passed against the artifact being shared.
- [ ] `release-manifest.json` version, DMG size, and DMG SHA-256 were reviewed.

Notes:

```text

```

## Manual Results

| Area | Result | Notes |
| --- | --- | --- |
| Install from DMG | [ ] Pass [ ] Fail [ ] N/A | |
| Launch installed `c4lens.app` | [ ] Pass [ ] Fail [ ] N/A | |
| Status bar shows expected version | [ ] Pass [ ] Fail [ ] N/A | |
| Open local repository | [ ] Pass [ ] Fail [ ] N/A | |
| `c4lens init` creates `c4/model.yml` and `c4/schema.json` | [ ] Pass [ ] Fail [ ] N/A | |
| `c4lens schema` restores bundled editor schema | [ ] Pass [ ] Fail [ ] N/A | |
| `c4lens doctor` reports repository readiness | [ ] Pass [ ] Fail [ ] N/A | |
| Validate valid model | [ ] Pass [ ] Fail [ ] N/A | |
| Invalid model keeps last valid canvas and shows path/line/column details | [ ] Pass [ ] Fail [ ] N/A | |
| Scan updates source counts | [ ] Pass [ ] Fail [ ] N/A | |
| Generate review/apply writes `c4/model.generated.yml` | [ ] Pass [ ] Fail [ ] N/A | |
| Generated provenance is visible | [ ] Pass [ ] Fail [ ] N/A | |
| Search opens elements, files, and symbols | [ ] Pass [ ] Fail [ ] N/A | |
| Jump to code opens source location | [ ] Pass [ ] Fail [ ] N/A | |
| Export SVG/PDF/PNG succeeds | [ ] Pass [ ] Fail [ ] N/A | |
| Light and dark themes render correctly | [ ] Pass [ ] Fail [ ] N/A | |
| Minimum window size remains usable | [ ] Pass [ ] Fail [ ] N/A | |
| `c4lens --version` matches app version | [ ] Pass [ ] Fail [ ] N/A | |

## Blockers

List any issue that should prevent sharing the candidate:

```text

```

## Follow-Ups

List non-blocking issues or release-note clarifications:

```text

```
