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
mkdir -p "$repo/src/web" "$repo/src/shared"
mkdir -p "$repo/src/pyapi" "$repo/src/pydomain"
mkdir -p "$repo/src/ruby_web" "$repo/src/ruby_domain"
mkdir -p "$repo/src/goapi" "$repo/src/godomain"
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
cat > "$repo/src/web/index.ts" <<'TS'
import { Shared } from "../shared";

export function boot(shared: Shared) {
  return shared;
}
TS
cat > "$repo/src/shared/index.ts" <<'TS'
export interface Shared {
  id: string;
}
TS
cat > "$repo/src/pyapi/service.py" <<'PY'
from src.pydomain import model

def handle():
    return model.Thing()
PY
cat > "$repo/src/pydomain/__init__.py" <<'PY'
class Thing:
    pass
PY
cat > "$repo/src/ruby_web/service.rb" <<'RB'
require_relative "../ruby_domain/model"

class Service
end
RB
cat > "$repo/src/ruby_domain/model.rb" <<'RB'
class Model
end
RB
cat > "$repo/src/goapi/main.go" <<'GO'
package goapi

import "../godomain"

func Handle() {}
GO
cat > "$repo/src/godomain/model.go" <<'GO'
package godomain

type Thing struct {}
GO

run_rust cargo build -p c4lens-cli
cli="./target/debug/c4lens"

run_cli() {
  HOME="$home" C4LENS_INDEX_DIR="$index_dir" "$cli" "$@"
}

run_cli validate --repo "$repo" --json >/dev/null
scan_json="$tmp_root/scan.json"
run_cli scan --repo "$repo" --json > "$scan_json"
node -e 'const summary = require(process.argv[1]); if (summary.symbols < 10 || summary.imports < 5) { throw new Error(`expected multi-language symbols/imports, got ${summary.symbols}/${summary.imports}`); }' "$scan_json"
generated_json="$tmp_root/generated.json"
run_cli generate --repo "$repo" --scan --json > "$generated_json"
node -e 'const payload = require(process.argv[1]); const count = (payload.generatedYaml.match(/description: Imports/g) || []).length; if (count < 5) { throw new Error(`expected at least 5 generated import relationships, got ${count}`); }' "$generated_json"
run_cli generate --repo "$repo" --write >/dev/null
run_cli generate --repo "$repo" --check --json >/dev/null
test -f "$repo/c4/model.generated.yml"
test -f "$repo/c4/schema.json"

npm --workspace app run test -- App.e2e.test.tsx

printf '%s\n' "MVP smoke passed"
