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
manifest_bundle="$(mktemp -d)"
cleanup() {
  rm -f "$metadata_path"
  rm -rf "$manifest_bundle"
}
trap cleanup EXIT

run_rust cargo metadata --format-version 1 --no-deps > "$metadata_path"

node - "$metadata_path" <<'NODE'
const fs = require("node:fs");

const metadataPath = process.argv[2];
const cargo = JSON.parse(fs.readFileSync(metadataPath, "utf8"));
const rootPackage = JSON.parse(fs.readFileSync("package.json", "utf8"));
const appPackage = JSON.parse(fs.readFileSync("app/package.json", "utf8"));
const appReleaseSource = fs.readFileSync("app/src/release.ts", "utf8");
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
const appVersion = appReleaseSource.match(/APP_VERSION\s*=\s*"([^"]+)"/)?.[1];

expectEqual("Tauri productName", tauriConfig.productName, "c4lens");
expectEqual("Tauri identifier", tauriConfig.identifier, "com.fuentesjr.c4lens");
expectEqual("Tauri bundle targets", tauriConfig.bundle?.targets, "all");
expectEqual("Tauri frontendDist", tauriConfig.build?.frontendDist, "../../app/dist");
expectEqual("app package version", appPackage.version, version);
expectEqual("renderer release version", appVersion, version);
expectEqual("package manifest script", rootPackage.scripts?.["package:manifest"], "bash scripts/write_macos_release_manifest.sh");

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
expectIncludes("MVP release checklist", releaseChecklist, "release-manifest.json");
expectIncludes("MVP release notes", releaseNotes, `Version: ${version}`);
expectIncludes("MVP release notes", releaseNotes, expectedDmg);
expectIncludes("MVP release notes", releaseNotes, "release-manifest.json");

if (errors.length > 0) {
  for (const error of errors) {
    console.error(error);
  }
  process.exit(1);
}
NODE

version="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").version')"
mkdir -p "$manifest_bundle/macos/c4lens.app/Contents/MacOS" "$manifest_bundle/dmg"
printf '%s\n' "demo app binary" > "$manifest_bundle/macos/c4lens.app/Contents/MacOS/c4lens"
printf '%s\n' "demo dmg bytes" > "$manifest_bundle/dmg/c4lens_${version}_universal.dmg"
bash scripts/write_macos_release_manifest.sh \
  "$manifest_bundle" \
  "$manifest_bundle/macos/c4lens.app" \
  "$manifest_bundle/dmg/c4lens_${version}_universal.dmg" \
  >/dev/null

node - "$manifest_bundle/release-manifest.json" "$version" <<'NODE'
const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const [manifestPath, version] = process.argv.slice(2);
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const manifestRoot = path.dirname(manifestPath);
const dmgPath = path.join(manifestRoot, manifest.artifacts.dmg.path);
const dmgSha256 = crypto.createHash("sha256").update(fs.readFileSync(dmgPath)).digest("hex");

if (manifest.version !== version) {
  throw new Error(`expected manifest version ${version}, got ${manifest.version}`);
}
if (manifest.artifacts.app.path !== "macos/c4lens.app") {
  throw new Error(`unexpected app path ${manifest.artifacts.app.path}`);
}
if (manifest.artifacts.dmg.sha256 !== dmgSha256) {
  throw new Error("manifest checksum does not match fixture DMG");
}
NODE

printf '%s\n' "Release metadata check passed"
