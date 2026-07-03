# Contributing

## Setup

Install JavaScript dependencies from the repository root:

```sh
npm install
```

Use Rust 1.96.0 for local checks when possible. If `mise` is installed,
`scripts/check.sh` will run Rust commands through `mise exec rust@1.96.0 --`.
Otherwise, make sure the active `cargo` toolchain matches the project.

## Before Committing

Run the full project gate:

```sh
npm run check:all
```

This is the same command used by CI. It runs Rust formatting, Clippy with
warnings denied, Rust tests, TypeScript checking, Vitest, and whitespace checks.

## Development Notes

- Keep generated overlay behavior centralized in `c4lens-core`.
- Keep Java support out of the active codebase.
- Prefer focused module extractions over broad rewrites.
- Add tests near the behavior being changed, and use shared test helpers when
  they keep setup readable.
