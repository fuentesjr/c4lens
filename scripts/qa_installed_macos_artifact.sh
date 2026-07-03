#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [[ "$(uname -s)" != "Darwin" ]]; then
  printf 'Installed macOS artifact QA must run on macOS; current OS is %s.\n' "$(uname -s)" >&2
  exit 1
fi

bundle_root="${1:-target/universal-apple-darwin/release/bundle}"
product_name="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").productName')"
version="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").version')"
identifier="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").identifier')"
dmg_path="$bundle_root/dmg/${product_name}_${version}_universal.dmg"
manifest_path="$bundle_root/release-manifest.json"

bash scripts/verify_macos_artifacts.sh "$bundle_root" >/dev/null

if [[ ! -f "$dmg_path" ]]; then
  printf 'DMG artifact not found: %s\n' "$dmg_path" >&2
  exit 1
fi

if [[ ! -f "$manifest_path" ]]; then
  printf 'Release manifest not found: %s\n' "$manifest_path" >&2
  exit 1
fi

mount_root="$(mktemp -d "${TMPDIR:-/tmp}/c4lens-dmg-mount.XXXXXX")"
install_root="$(mktemp -d "${TMPDIR:-/tmp}/c4lens-installed-app.XXXXXX")"
attach_error="$install_root/hdiutil-attach.err"
attach_output="$install_root/hdiutil-attach.out"
mounted=0
mount_status=""

cleanup() {
  if [[ "$mounted" -eq 1 ]]; then
    hdiutil detach "$mount_root" -quiet || true
  fi
}
trap cleanup EXIT

if hdiutil attach "$dmg_path" -readonly -nobrowse -mountpoint "$mount_root" >"$attach_output" 2>"$attach_error"; then
  mounted=1
  app_source="$(find "$mount_root" -maxdepth 2 -type d -name "${product_name}.app" -print -quit)"
  mount_status="mounted"
  if [[ -z "$app_source" ]]; then
    printf 'Mounted app not found in DMG: %s\n' "$dmg_path" >&2
    exit 1
  fi
else
  app_source="$bundle_root/macos/${product_name}.app"
  attach_message="$(cat "$attach_error" "$attach_output" | tr '\n' ' ' | sed 's/[[:space:]]*$//')"
  if [[ -z "$attach_message" ]]; then
    attach_message="hdiutil attach failed without diagnostic output"
  fi
  mount_status="attach unavailable: $attach_message"
  if [[ ! -d "$app_source" ]]; then
    printf 'DMG attach failed and packaged app fallback was not found: %s\n' "$app_source" >&2
    printf 'hdiutil: %s\n' "$mount_status" >&2
    exit 1
  fi
fi

installed_app="$install_root/${product_name}.app"
ditto "$app_source" "$installed_app"

info_plist="$installed_app/Contents/Info.plist"
if [[ ! -f "$info_plist" ]]; then
  printf 'Installed Info.plist not found: %s\n' "$info_plist" >&2
  exit 1
fi

plist_value() {
  /usr/libexec/PlistBuddy -c "Print :$1" "$info_plist"
}

app_name="$(plist_value CFBundleName)"
app_identifier="$(plist_value CFBundleIdentifier)"
app_short_version="$(plist_value CFBundleShortVersionString)"
app_bundle_version="$(plist_value CFBundleVersion)"
app_executable="$(plist_value CFBundleExecutable)"
app_binary="$installed_app/Contents/MacOS/$app_executable"

if [[ "$app_name" != "$product_name" ]]; then
  printf 'Unexpected installed CFBundleName: expected %s, got %s\n' "$product_name" "$app_name" >&2
  exit 1
fi

if [[ "$app_identifier" != "$identifier" ]]; then
  printf 'Unexpected installed CFBundleIdentifier: expected %s, got %s\n' "$identifier" "$app_identifier" >&2
  exit 1
fi

if [[ "$app_short_version" != "$version" || "$app_bundle_version" != "$version" ]]; then
  printf 'Unexpected installed version: short=%s bundle=%s expected=%s\n' "$app_short_version" "$app_bundle_version" "$version" >&2
  exit 1
fi

if [[ ! -x "$app_binary" ]]; then
  printf 'Installed app executable not found or not executable: %s\n' "$app_binary" >&2
  exit 1
fi

plutil -lint "$info_plist" >/dev/null

archs="$(lipo -archs "$app_binary")"
case " $archs " in
  *" x86_64 "* ) ;;
  * ) printf 'Installed executable is missing x86_64 arch: %s\n' "$archs" >&2; exit 1 ;;
esac
case " $archs " in
  *" arm64 "* ) ;;
  * ) printf 'Installed executable is missing arm64 arch: %s\n' "$archs" >&2; exit 1 ;;
esac

node - "$manifest_path" "$dmg_path" "$version" <<'NODE'
const crypto = require("node:crypto");
const fs = require("node:fs");

const [manifestPath, dmgPath, version] = process.argv.slice(2);
const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
const dmgSha256 = crypto.createHash("sha256").update(fs.readFileSync(dmgPath)).digest("hex");

if (manifest.version !== version) {
  throw new Error(`unexpected manifest version ${manifest.version}`);
}
if (manifest.platform !== "macos-universal") {
  throw new Error(`unexpected manifest platform ${manifest.platform}`);
}
if (manifest.artifacts?.dmg?.sha256 !== dmgSha256) {
  throw new Error("manifest DMG checksum does not match artifact");
}
if (!manifest.artifacts?.app?.path?.endsWith(".app")) {
  throw new Error("manifest app path is missing");
}
NODE

printf '%s\n' "Installed macOS artifact QA passed"
printf '  dmg: %s\n' "$dmg_path"
printf '  dmg mount: %s\n' "$mount_status"
printf '  installed app: %s\n' "$installed_app"
printf '  version: %s\n' "$version"
printf '  archs: %s\n' "$archs"
