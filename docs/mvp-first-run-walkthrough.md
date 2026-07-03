# MVP First-Run Walkthrough

Use this walkthrough for a clean first-run check of the internal macOS MVP
candidate. It pairs the disposable demo repository with the CLI and desktop
workflows covered by the automated smoke tests.

## Prepare A Demo Repository

Create a mixed-language repository fixture:

```sh
npm run demo:mvp-repo -- /tmp/c4lens-mvp-demo
```

The fixture includes C4 model files plus TypeScript, Python, Ruby, Go, and Rust
source files so scan and generation results are representative of the MVP
language set.

## Validate The CLI Path

For the repeatable first-run CLI gate, run:

```sh
npm run qa:first-run -- /tmp/c4lens-mvp-demo
```

This creates the demo repository, builds the CLI, isolates `HOME` and
`C4LENS_INDEX_DIR` under temporary QA state, verifies that `doctor` requests a
schema refresh before first-run setup is complete, refreshes the schema, and
then exercises validation, scan, generation preview, generation write, drift
check, and final validation.

To run the same flow manually from a local build, isolate c4lens state before
using the CLI against the demo repository:

```sh
export C4LENS_QA_HOME=/tmp/c4lens-mvp-qa-home
export C4LENS_INDEX_DIR=/tmp/c4lens-mvp-qa-indexes
mkdir -p "$C4LENS_QA_HOME" "$C4LENS_INDEX_DIR"
cargo build -p c4lens-cli
HOME="$C4LENS_QA_HOME" C4LENS_INDEX_DIR="$C4LENS_INDEX_DIR" target/debug/c4lens doctor --repo /tmp/c4lens-mvp-demo
HOME="$C4LENS_QA_HOME" C4LENS_INDEX_DIR="$C4LENS_INDEX_DIR" target/debug/c4lens schema --repo /tmp/c4lens-mvp-demo
HOME="$C4LENS_QA_HOME" C4LENS_INDEX_DIR="$C4LENS_INDEX_DIR" target/debug/c4lens doctor --repo /tmp/c4lens-mvp-demo
HOME="$C4LENS_QA_HOME" C4LENS_INDEX_DIR="$C4LENS_INDEX_DIR" target/debug/c4lens validate --repo /tmp/c4lens-mvp-demo
HOME="$C4LENS_QA_HOME" C4LENS_INDEX_DIR="$C4LENS_INDEX_DIR" target/debug/c4lens scan --repo /tmp/c4lens-mvp-demo --json
HOME="$C4LENS_QA_HOME" C4LENS_INDEX_DIR="$C4LENS_INDEX_DIR" target/debug/c4lens generate --repo /tmp/c4lens-mvp-demo --scan --json
HOME="$C4LENS_QA_HOME" C4LENS_INDEX_DIR="$C4LENS_INDEX_DIR" target/debug/c4lens generate --repo /tmp/c4lens-mvp-demo --write
HOME="$C4LENS_QA_HOME" C4LENS_INDEX_DIR="$C4LENS_INDEX_DIR" target/debug/c4lens generate --repo /tmp/c4lens-mvp-demo --check
HOME="$C4LENS_QA_HOME" C4LENS_INDEX_DIR="$C4LENS_INDEX_DIR" target/debug/c4lens validate --repo /tmp/c4lens-mvp-demo
```

Expected results:

- The first `doctor` reports that `c4/schema.json` should be refreshed.
- `schema` creates `c4/schema.json`.
- The second `doctor` reports the repository as ready.
- `scan --json` reports symbols and imports across the demo source files.
- `generate --scan --json` includes generated import relationships.
- `generate --write` creates `c4/model.generated.yml` and refreshes
  `c4/schema.json`.
- The final `validate` succeeds for the effective model.

## Validate The Desktop Path

For the aggregate release-candidate gate before human desktop interaction, run:

```sh
npm run qa:release-candidate
```

Before the human desktop interaction pass, run the installed-artifact gate
against the local release bundle:

```sh
npm run qa:installed-macos
```

This mounts the DMG, copies `c4lens.app` to a temporary install directory,
checks installed bundle metadata, verifies the universal executable, and
cross-checks `release-manifest.json`.

For a CI candidate, verify the pushed commit artifact and regenerate the GUI
handoff before installing:

```sh
npm run qa:current-ci-artifact -- <commit-sha>
npm run qa:gui-handoff -- <workflow-run-id> <commit-sha>
```

Launch the app in development or install the release candidate from the DMG:

```sh
npm run tauri:dev
```

Then exercise the first-run desktop path:

- Open `/tmp/c4lens-mvp-demo`.
- Confirm the status bar shows the expected app version.
- Confirm the model loads without validation errors.
- Run Scan and confirm source counts update.
- Run Generate, review the diff, apply it, and confirm the generated overlay is
  saved.
- Use search to open an element, an indexed file, and an indexed symbol.
- Jump to code from the detail panel.
- Export SVG, PDF, and PNG from the current view.
- Toggle light and dark themes.
- Resize to the minimum supported window size and confirm the canvas and detail
  panel remain usable.

## Reset

Delete the disposable repository when finished:

```sh
rm -rf /tmp/c4lens-mvp-demo
```

For the formal release pass, record results with the
[MVP manual QA template](mvp-manual-qa-template.md).
