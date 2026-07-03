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

From a local build, use the CLI against the demo repository:

```sh
cargo build -p c4lens-cli
target/debug/c4lens doctor --repo /tmp/c4lens-mvp-demo
target/debug/c4lens validate --repo /tmp/c4lens-mvp-demo
target/debug/c4lens scan --repo /tmp/c4lens-mvp-demo --json
target/debug/c4lens generate --repo /tmp/c4lens-mvp-demo --scan --json
target/debug/c4lens generate --repo /tmp/c4lens-mvp-demo --write
target/debug/c4lens generate --repo /tmp/c4lens-mvp-demo --check
target/debug/c4lens validate --repo /tmp/c4lens-mvp-demo
```

Expected results:

- `doctor` reports the repository as ready.
- `scan --json` reports symbols and imports across the demo source files.
- `generate --scan --json` includes generated import relationships.
- `generate --write` creates `c4/model.generated.yml` and refreshes
  `c4/schema.json`.
- The final `validate` succeeds for the effective model.

## Validate The Desktop Path

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
