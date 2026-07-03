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

metadata_path="$(mktemp)"
cleanup() {
  rm -f "$metadata_path"
}
trap cleanup EXIT

run_rust cargo metadata --format-version 1 --no-deps > "$metadata_path"

node - "$metadata_path" <<'NODE'
const fs = require("node:fs");

const metadataPath = process.argv[2];
const cargo = JSON.parse(fs.readFileSync(metadataPath, "utf8"));
const appPackage = JSON.parse(fs.readFileSync("app/package.json", "utf8"));
const tauriConfig = JSON.parse(fs.readFileSync("crates/c4lens-tauri/tauri.conf.json", "utf8"));
const releaseChecklist = fs.readFileSync("docs/mvp-release-checklist.md", "utf8");
const releaseNotes = fs.readFileSync("docs/mvp-release-notes.md", "utf8");
const errors = [];

function expectEqual(label, actual, expected) {
  if (actual !== expected) {
    errors.push(`${label}: expected ${expected}, got ${actual}`);
  }
}

function expectIncludes(label, content, expected) {
  if (!content.includes(expected)) {
    errors.push(`${label}: missing ${expected}`);
  }
}

const packages = new Map(cargo.packages.map((pkg) => [pkg.name, pkg]));
const version = tauriConfig.version;

expectEqual("Tauri productName", tauriConfig.productName, "c4lens");
expectEqual("Tauri identifier", tauriConfig.identifier, "com.fuentesjr.c4lens");
expectEqual("Tauri bundle targets", tauriConfig.bundle?.targets, "all");
expectEqual("Tauri frontendDist", tauriConfig.build?.frontendDist, "../../app/dist");
expectEqual("app package version", appPackage.version, version);

for (const packageName of ["c4lens-core", "c4lens-cli", "c4lens-tauri"]) {
  const pkg = packages.get(packageName);
  if (!pkg) {
    errors.push(`Cargo package missing: ${packageName}`);
    continue;
  }
  expectEqual(`${packageName} version`, pkg.version, version);
}

const cliPackage = packages.get("c4lens-cli");
const cliTargets = new Set(
  (cliPackage?.targets ?? [])
    .filter((target) => target.kind.includes("bin"))
    .map((target) => target.name),
);
if (!cliTargets.has("c4lens")) {
  errors.push("c4lens-cli must expose a c4lens binary target");
}

const expectedDmg = `c4lens_${version}_universal.dmg`;
expectIncludes("MVP release checklist", releaseChecklist, expectedDmg);
expectIncludes("MVP release notes", releaseNotes, `Version: ${version}`);
expectIncludes("MVP release notes", releaseNotes, expectedDmg);

if (errors.length > 0) {
  for (const error of errors) {
    console.error(error);
  }
  process.exit(1);
}
NODE

printf '%s\n' "Release metadata check passed"
