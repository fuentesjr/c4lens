# CLI Quickstart

Use this flow when validating a repository from the command line before opening
it in the desktop app or sharing it with another reviewer. Commands accept
`--repo`; omit it to use the current working directory.

## First Repository Setup

Create the authored model and editor schema:

```sh
c4lens init --repo /path/to/repo --name "My System"
```

This creates:

```text
/path/to/repo/c4/model.yml
/path/to/repo/c4/schema.json
```

`init` does not overwrite an existing authored model. To refresh only the
editor schema later, run:

```sh
c4lens schema --repo /path/to/repo
```

## Readiness Check

Run the non-mutating repository health check:

```sh
c4lens doctor --repo /path/to/repo
```

`doctor` reports whether the authored model, editor schema, generated overlay,
and validation state are ready. Use JSON output for automation:

```sh
c4lens doctor --repo /path/to/repo --json
```

## Validate And Index

Validate the effective C4 model:

```sh
c4lens validate --repo /path/to/repo
```

Scan the repository and inspect indexed source counts:

```sh
c4lens scan --repo /path/to/repo --json
```

Use `--force` when you need to rebuild the source index from scratch:

```sh
c4lens scan --repo /path/to/repo --force --json
```

## Preview And Apply Generation

Preview generated overlay YAML without writing files:

```sh
c4lens generate --repo /path/to/repo --scan --json
```

Write the generated overlay after reviewing the candidate output:

```sh
c4lens generate --repo /path/to/repo --write
```

The write path updates:

```text
/path/to/repo/c4/model.generated.yml
/path/to/repo/c4/schema.json
```

Confirm the written generated overlay still matches the current scan-derived
candidate:

```sh
c4lens generate --repo /path/to/repo --check
```

Finish by validating the effective authored plus generated model:

```sh
c4lens validate --repo /path/to/repo
```

## Exit Code Contract

- `0`: command completed successfully.
- `1`: repository content needs attention, such as validation errors or drift.
- `3`: the command could not access the repository or acquire the write lock.

Prefer `--json` in scripts so callers can inspect issue codes and paths without
parsing human-readable output.
