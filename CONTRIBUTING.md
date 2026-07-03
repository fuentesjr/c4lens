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
warnings denied, Rust tests, release metadata checks, the MVP documentation
contract check, TypeScript checking, Vitest, and whitespace checks.

## Branch And Scope Hygiene

Keep unsupported language and runtime experiments on short-lived branches until
they are explicitly accepted into the roadmap. If out-of-scope work has already
been published, prefer an explicit revert unless maintainers choose to rewrite
the branch before review or release.

Before publishing a branch, make the history strategy intentional: either keep
the revert as part of the record, or squash/rewrite unpublished local history so
reviewers do not have to reason about reverted experiments.

## Development Notes

- Keep generated overlay behavior centralized in `c4lens-core`.
- Keep Java support out of the active codebase.
- Prefer focused module extractions over broad rewrites.
- Add tests near the behavior being changed, and use shared test helpers when
  they keep setup readable.
