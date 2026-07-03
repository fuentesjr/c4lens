#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

run_rust() {
  if command -v mise >/dev/null 2>&1; then
    mise exec rust@1.96.0 -- "$@"
  else
    "$@"
  fi
}

tmp_root="$(mktemp -d)"
cleanup() {
  rm -rf "$tmp_root"
}
trap cleanup EXIT

repo="$tmp_root/repo"
home="$tmp_root/home"
index_dir="$tmp_root/indexes"
mkdir -p "$home" "$repo/c4" "$repo/src/api" "$repo/src/domain"
cat > "$repo/c4/model.yml" <<'YAML'
name: Smoke Repo
YAML
cat > "$repo/Cargo.toml" <<'TOML'
[package]
name = "smoke-service"
version = "0.1.0"
edition = "2021"
TOML
cat > "$repo/src/api/mod.rs" <<'RS'
use crate::domain::Thing;

pub fn handle() {}
RS
cat > "$repo/src/domain/mod.rs" <<'RS'
pub struct Thing;
RS

run_rust cargo build -p c4lens-cli
cli="./target/debug/c4lens-cli"

run_cli() {
  HOME="$home" C4LENS_INDEX_DIR="$index_dir" "$cli" "$@"
}

run_cli validate --repo "$repo" --json >/dev/null
run_cli scan --repo "$repo" --json >/dev/null
run_cli generate --repo "$repo" --scan --json >/dev/null
run_cli generate --repo "$repo" --write >/dev/null
run_cli generate --repo "$repo" --check --json >/dev/null
test -f "$repo/c4/model.generated.yml"
test -f "$repo/c4/schema.json"

printf '%s\n' "MVP smoke passed"
