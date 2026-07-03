# MVP Installed GUI QA Handoff - 2026-07-03

Candidate commit: `26e8e0432149b0d5d5e7e889c78a8001ab0a51d2`

Workflow run: `28686171140`

Artifact name:
`c4lens-0.1.0-macos-universal-26e8e0432149b0d5d5e7e889c78a8001ab0a51d2`

Status: ready for human installed-app interaction pass.

## Automated Gate Context

- CI run `28686171140` passed and uploaded the expected macOS universal
  artifact.
- `npm run qa:installed-macos` passed locally against the release bundle. The
  sandboxed terminal could not attach the DMG (`Device not configured`), so the
  gate verified the packaged app fallback from the release bundle.
- `npm run qa:first-run` passed locally for the disposable first-run CLI flow.

## Human Interaction Checklist

Use the CI artifact or a fresh local `npm run smoke:release` artifact, install
from the DMG on a current supported macOS machine, then record results in
`docs/mvp-manual-qa-template.md`.

| Workflow | Result | Notes |
| --- | --- | --- |
| DMG mounts in Finder | Not run | Requires human GUI session. |
| `c4lens.app` copies to Applications or a temporary install directory | Not run | Requires human GUI session. |
| App launches from installed location | Not run | Requires human GUI session. |
| Status bar shows version `0.1.0` | Not run | Requires human GUI session. |
| Demo repository opens successfully | Not run | Use `/tmp/c4lens-mvp-demo` or equivalent. |
| Scan updates source counts | Not run | Requires app interaction. |
| Generate preview, diff review, and apply succeed | Not run | Confirm `c4/model.generated.yml` is written. |
| Search opens an element, file, and symbol | Not run | Exercise keyboard navigation. |
| Jump to code opens source location | Not run | Requires installed app permissions. |
| Export SVG/PDF/PNG succeeds | Not run | Verify saved files exist and are non-empty. |
| Light/dark theme toggle persists during session | Not run | Requires app interaction. |
| Minimum-size resize remains usable | Not run | Requires app interaction. |

## Blocker/High Findings

None from automated gates. The human installed-app interaction pass has not yet
been run.
