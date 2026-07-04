# MVP Installed GUI QA Handoff - 2026-07-03

Candidate commit: `dcba6e204cfd4450b6fd1753ee368ec34ddc58d9`

Workflow run: `28691766065`

Artifact name:
`c4lens-0.1.0-macos-universal-dcba6e204cfd4450b6fd1753ee368ec34ddc58d9`

Artifact expiration: `2026-07-18T02:18:17Z`

Artifact size: 24593814 bytes

Status: ready for human installed-app interaction pass.

## Automated Gate Context

- CI run `28691766065` uploaded the expected macOS universal artifact.
- `npm run qa:current-ci-artifact -- dcba6e204cfd4450b6fd1753ee368ec34ddc58d9` verifies this run and
  artifact metadata.
- `npm run qa:prepare-ci-candidate -- 28691766065 dcba6e204cfd4450b6fd1753ee368ec34ddc58d9` downloads and
  verifies a local candidate bundle under `target/mvp-candidates/`.
- `npm run qa:release-candidate` remains the local pre-human-review gate for
  first-run CLI QA, installed macOS artifact QA, and MVP docs contract checks.

## Human Interaction Checklist

Download the CI artifact or rebuild locally with `npm run smoke:release`,
install from the DMG on a current supported macOS machine, then record results
in `docs/mvp-manual-qa-template.md`.

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
