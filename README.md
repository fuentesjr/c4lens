# c4lens

c4lens is a local Tauri desktop app and headless CLI for navigating a C4 model
stored in a repository. The authored model lives in `c4/model.yml`; generated,
disposable overlay data lives in `c4/model.generated.yml`.

The project is intentionally local-first:

- Rust core for filesystem access, validation, scanning, generation, indexing,
  and native desktop commands.
- React/Vite renderer for navigation, derived C4 views, ELK layout, and canvas
  rendering.
- CLI commands for repo initialization, schema refresh, health checks,
  validation, scanning, and generation.
- Best-effort symbol and import indexing for the MVP language set:
  TypeScript/JavaScript, Ruby, Rust, Go, and Python.

## Repository Layout

- `crates/c4lens-core`: model loading, validation, scanning, indexing, and
  generation.
- `crates/c4lens-cli`: `c4lens` command-line entrypoint.
- `crates/c4lens-tauri`: Tauri command/event layer and desktop shell.
- `app`: React/Vite renderer.
- `docs`: design and implementation spec.

## Local Development

Install JavaScript dependencies once:

```sh
npm install
```

Run the renderer:

```sh
npm run dev
```

Run the Tauri app in development:

```sh
npm run tauri:dev
```

Initialize a repository for c4lens:

```sh
c4lens init --repo /path/to/repo --name "My System"
```

This creates `c4/model.yml` and refreshes `c4/schema.json` for editor
autocomplete. To refresh only the editor schema later:

```sh
c4lens schema --repo /path/to/repo
```

Check repository readiness before opening it in the app or sharing it:

```sh
c4lens doctor --repo /path/to/repo
```

For the full command-line flow from repository initialization through generated
overlay drift checking, see the [CLI quickstart](docs/cli-quickstart.md).

## Quality Gates

Run the full local check before committing:

```sh
npm run check:all
```

GitHub Actions runs the same command on pushes to `main` and pull requests.
Keep local and CI behavior aligned by updating `scripts/check.sh` when the gate
changes.

This runs:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `npm run check:release-metadata`
- `npm run check:tauri-security`
- `npm run check:mvp-docs`
- `npm run check`
- `npm run test`
- `git diff --check`

When `mise` is available, the script runs Rust commands through
`mise exec rust@1.96.0 -- ...`; otherwise it uses the active `cargo` on `PATH`.

For targeted checks:

```sh
npm run check
npm run check:mvp-docs
npm run check:release-metadata
npm run check:tauri-security
npm run test
npm run package:manifest
npm run smoke:mvp
npm run smoke:release
npm run qa:first-run
npm run qa:installed-macos
```

Create a disposable mixed-language repo for manual MVP checks:

```sh
npm run demo:mvp-repo -- /tmp/c4lens-mvp-demo
```

`npm run smoke:mvp` creates a temporary repository and exercises the CLI MVP
path: validate, scan, generate preview, generated overlay write, generated
drift check, mixed-language import relationship generation, and the renderer
E2E workflow tests.

`npm run smoke:release` runs the full quality gate, MVP smoke, unsigned
universal macOS build, and artifact verification. It must run on macOS.

`npm run qa:first-run` creates a disposable demo repository, isolates c4lens
state under temporary QA directories, and exercises the first-run CLI path from
schema refresh through scan, generation, write, drift check, and validation.

`npm run qa:installed-macos` verifies the local DMG as an installable artifact:
it mounts the DMG, copies `c4lens.app` to a temporary install directory, checks
bundle metadata, verifies universal executable architectures, and cross-checks
the release manifest.

## Reference Docs

- [Project tracker](PROJECT_TRACKER.md)
- [Contributing](CONTRIBUTING.md)
- [Product roadmap](docs/roadmap.md)
- [CLI quickstart](docs/cli-quickstart.md)
- [Packaging](docs/packaging.md)
- [Release artifact handling](docs/release-artifact-handling.md)
- [MVP release checklist](docs/mvp-release-checklist.md)
- [MVP first-run walkthrough](docs/mvp-first-run-walkthrough.md)
- [MVP manual QA template](docs/mvp-manual-qa-template.md)
- [MVP QA triage](docs/mvp-qa-triage.md)
- [MVP release notes](docs/mvp-release-notes.md)
- [Signing and notarization decision](docs/signing-notarization.md)
- [Desktop design](docs/design/c4lens-desktop-design.md)
- [Desktop implementation spec](docs/spec/c4lens-desktop-spec.md)
