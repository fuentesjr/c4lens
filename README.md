# c4lens

c4lens is a local Tauri desktop app and headless CLI for navigating a C4 model
stored in a repository. The authored model lives in `c4/model.yml`; generated,
disposable overlay data lives in `c4/model.generated.yml`.

The project is intentionally local-first:

- Rust core for filesystem access, validation, scanning, generation, indexing,
  and native desktop commands.
- React/Vite renderer for navigation, derived C4 views, ELK layout, and canvas
  rendering.
- CLI commands for headless validation, scanning, and generation.

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
- `npm run check`
- `npm run test`
- `git diff --check`

When `mise` is available, the script runs Rust commands through
`mise exec rust@1.96.0 -- ...`; otherwise it uses the active `cargo` on `PATH`.

For targeted checks:

```sh
npm run check
npm run test
npm run smoke:mvp
```

`npm run smoke:mvp` creates a temporary repository and exercises the CLI MVP
path: validate, scan, generate preview, generated overlay write, generated
drift check, and the renderer E2E workflow tests.

## Reference Docs

- [Contributing](CONTRIBUTING.md)
- [Product roadmap](docs/roadmap.md)
- [Packaging](docs/packaging.md)
- [Desktop design](docs/design/c4lens-desktop-design.md)
- [Desktop implementation spec](docs/spec/c4lens-desktop-spec.md)
