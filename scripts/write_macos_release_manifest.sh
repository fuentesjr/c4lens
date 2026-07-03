#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

bundle_root="${1:-target/universal-apple-darwin/release/bundle}"
app_path="${2:-$bundle_root/macos/c4lens.app}"
dmg_path="${3:-}"

product_name="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").productName')"
version="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").version')"
identifier="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").identifier')"

if [[ -z "$dmg_path" ]]; then
  expected_dmg_path="$bundle_root/dmg/${product_name}_${version}_universal.dmg"
  if [[ -f "$expected_dmg_path" ]]; then
    dmg_path="$expected_dmg_path"
  else
    dmg_path="$(find "$bundle_root" -type f -name '*.dmg' ! -name 'rw.*.dmg' -print -quit)"
  fi
fi

if [[ ! -d "$app_path" ]]; then
  printf 'App bundle not found: %s\n' "$app_path" >&2
  exit 1
fi

if [[ -z "$dmg_path" || ! -f "$dmg_path" ]]; then
  printf 'DMG artifact not found under %s\n' "$bundle_root" >&2
  exit 1
fi

manifest_path="$bundle_root/release-manifest.json"
mkdir -p "$bundle_root"

node - "$manifest_path" "$bundle_root" "$app_path" "$dmg_path" "$product_name" "$version" "$identifier" <<'NODE'
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const [manifestPath, bundleRoot, appPath, dmgPath, productName, version, identifier] = process.argv.slice(2);

function relative(target) {
  return path.relative(bundleRoot, target).split(path.sep).join("/");
}

function fileSha256(target) {
  return crypto.createHash("sha256").update(fs.readFileSync(target)).digest("hex");
}

function countFiles(directory) {
  let count = 0;
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      count += countFiles(entryPath);
    } else if (entry.isFile()) {
      count += 1;
    }
  }
  return count;
}

const dmgStats = fs.statSync(dmgPath);
const manifest = {
  schemaVersion: 1,
  productName,
  version,
  identifier,
  platform: "macos-universal",
  artifacts: {
    app: {
      path: relative(appPath),
      fileCount: countFiles(appPath),
    },
    dmg: {
      path: relative(dmgPath),
      bytes: dmgStats.size,
      sha256: fileSha256(dmgPath),
    },
  },
};

fs.writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
NODE

printf 'Wrote release manifest: %s\n' "$manifest_path"
