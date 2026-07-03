# MVP First-Run QA Result - 2026-07-03

## Candidate

- Candidate version: 0.1.0
- Candidate branch: `main`
- Candidate commit under test before this QA fix: `ba9a730`
- QA scope: first-run CLI path plus renderer workflow smoke.
- Artifact source: local development checkout.

## Commands

```sh
npm run smoke:mvp
npm run qa:first-run
```

## Results

| Area | Result | Notes |
| --- | --- | --- |
| Renderer workflow smoke | Pass | `npm run smoke:mvp` passed, including renderer E2E coverage for search, validation, generation, jump-to-code, SVG export, PDF export, and PNG export. |
| First-run CLI QA | Pass | `npm run qa:first-run` passed after the walkthrough was corrected to refresh `c4/schema.json` before expecting `doctor` to report ready. |
| Blocker/high findings | None open | The reproducible findings were documentation/script gaps, fixed by adding the first-run QA script and updating the walkthrough. |

## Findings

The initial ad hoc walkthrough run exposed two gaps:

- The demo repository intentionally starts without `c4/schema.json`, so
  `doctor` correctly recommends running `schema` before reporting ready.
- CLI QA should isolate `HOME` and `C4LENS_INDEX_DIR` so lock and index writes
  do not depend on the operator's local application-support directory.

Both are now covered by `npm run qa:first-run`.

## Desktop Manual Coverage

The installed DMG desktop pass was not run in this terminal session. The
renderer workflow portion remains covered by `npm run smoke:mvp`; the installed
desktop pass remains the next human QA step before wider internal sharing.
