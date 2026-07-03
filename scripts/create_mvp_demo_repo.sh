#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

usage() {
  printf 'Usage: npm run demo:mvp-repo -- [destination]\n' >&2
}

if [[ "$#" -gt 1 ]]; then
  usage
  exit 64
fi

if [[ "$#" -eq 1 ]]; then
  repo="$1"
else
  repo="$(mktemp -d "${TMPDIR:-/tmp}/c4lens-mvp-demo.XXXXXX")"
fi

if [[ -e "$repo" && ! -d "$repo" ]]; then
  printf 'Destination exists and is not a directory: %s\n' "$repo" >&2
  exit 1
fi

if [[ -d "$repo" && -n "$(find "$repo" -mindepth 1 -maxdepth 1 -print -quit)" ]]; then
  printf 'Destination directory must be empty: %s\n' "$repo" >&2
  exit 1
fi

mkdir -p "$repo/c4" "$repo/src/api" "$repo/src/domain"
mkdir -p "$repo/src/web" "$repo/src/shared"
mkdir -p "$repo/src/pyapi" "$repo/src/pydomain"
mkdir -p "$repo/src/ruby_web" "$repo/src/ruby_domain"
mkdir -p "$repo/src/goapi" "$repo/src/godomain"

cat > "$repo/c4/model.yml" <<'YAML'
name: MVP Demo Repo
YAML

cat > "$repo/Cargo.toml" <<'TOML'
[package]
name = "demo-service"
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

printf 'Created MVP demo repo at %s\n' "$repo"
