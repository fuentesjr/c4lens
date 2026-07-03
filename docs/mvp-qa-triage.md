# MVP QA Triage

Use this process after each manual QA pass to decide whether an internal macOS
MVP candidate can be shared.

## Issue Classes

| Class | Meaning | Candidate Decision |
| --- | --- | --- |
| Blocker | Prevents launch, install, repository open, validation, scan, generation, export, or artifact verification. | Do not share. Fix and rerun `npm run smoke:release`. |
| High | Breaks a primary workflow but has a clear workaround. | Fix before broader internal sharing unless explicitly accepted. |
| Medium | Affects clarity, polish, or an edge case without blocking first-run validation. | Track as follow-up and disclose in candidate notes if visible. |
| Low | Cosmetic or documentation-only issue. | Track as follow-up. |

## Required Fields

Record each issue with:

- Candidate version.
- Candidate commit.
- Artifact name or local artifact path.
- macOS version and machine architecture.
- Reproduction steps.
- Expected result.
- Actual result.
- Screenshots or terminal output when useful.
- Proposed disposition: blocker, fix now, known limit, or backlog.

## Disposition Rules

- Blockers must be fixed before the candidate is called ready.
- Known MVP limits should match [PROJECT_TRACKER.md](../PROJECT_TRACKER.md) or
  [MVP release notes](mvp-release-notes.md).
- New non-blocking issues should be listed in the manual QA result notes.
- Any fix that touches product behavior must rerun `npm run check:all` and the
  relevant smoke command.
- Any fix that changes release packaging must rerun `npm run smoke:release`.

## Candidate Ready Criteria

A candidate can be shared internally when:

- `npm run smoke:release` passed on the candidate commit.
- Manual QA has no open blockers.
- High issues are fixed or explicitly accepted for internal-only sharing.
- Artifact version, commit, and DMG SHA-256 are recorded.
- Known limits are reflected in the release notes or project tracker.
