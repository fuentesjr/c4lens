#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

bundle_root="${1:-target/universal-apple-darwin/release/bundle}"
product_name="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").productName')"
version="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").version')"
identifier="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").identifier')"
expected_dmg_path="$bundle_root/dmg/${product_name}_${version}_universal.dmg"

if [[ ! -d "$bundle_root" ]]; then
  printf 'macOS bundle directory not found: %s\n' "$bundle_root" >&2
  exit 1
fi

app_path="$(find "$bundle_root" -type d -name 'c4lens.app' -print -quit)"
dmg_path=""

if [[ -f "$expected_dmg_path" ]]; then
  dmg_path="$expected_dmg_path"
else
  dmg_path="$(find "$bundle_root" -type f -name '*.dmg' ! -name 'rw.*.dmg' -print -quit)"
fi

if [[ -z "$app_path" ]]; then
  printf 'c4lens.app not found under %s\n' "$bundle_root" >&2
  exit 1
fi

if [[ -z "$dmg_path" ]]; then
  printf 'DMG artifact not found under %s\n' "$bundle_root" >&2
  exit 1
fi

if [[ ! -s "$dmg_path" ]]; then
  printf 'DMG artifact is empty: %s\n' "$dmg_path" >&2
  exit 1
fi

info_plist="$app_path/Contents/Info.plist"

if [[ ! -f "$info_plist" ]]; then
  printf 'Info.plist not found: %s\n' "$info_plist" >&2
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
app_binary="$app_path/Contents/MacOS/$app_executable"

if [[ "$app_name" != "$product_name" ]]; then
  printf 'Unexpected CFBundleName: expected %s, got %s\n' "$product_name" "$app_name" >&2
  exit 1
fi

if [[ "$app_identifier" != "$identifier" ]]; then
  printf 'Unexpected CFBundleIdentifier: expected %s, got %s\n' "$identifier" "$app_identifier" >&2
  exit 1
fi

if [[ "$app_short_version" != "$version" ]]; then
  printf 'Unexpected CFBundleShortVersionString: expected %s, got %s\n' "$version" "$app_short_version" >&2
  exit 1
fi

if [[ "$app_bundle_version" != "$version" ]]; then
  printf 'Unexpected CFBundleVersion: expected %s, got %s\n' "$version" "$app_bundle_version" >&2
  exit 1
fi

if [[ ! -x "$app_binary" ]]; then
  printf 'App executable not found or not executable: %s\n' "$app_binary" >&2
  exit 1
fi

if command -v plutil >/dev/null 2>&1; then
  plutil -lint "$info_plist" >/dev/null
fi

if command -v lipo >/dev/null 2>&1; then
  archs="$(lipo -archs "$app_binary")"
  case " $archs " in
    *" x86_64 "* ) ;;
    * ) printf 'App executable is missing x86_64 arch: %s\n' "$archs" >&2; exit 1 ;;
  esac
  case " $archs " in
    *" arm64 "* ) ;;
    * ) printf 'App executable is missing arm64 arch: %s\n' "$archs" >&2; exit 1 ;;
  esac
fi

if command -v hdiutil >/dev/null 2>&1; then
  hdiutil verify "$dmg_path" >/dev/null 2>&1
fi

printf 'Verified macOS artifacts:\n'
printf '  app: %s\n' "$app_path"
printf '  dmg: %s\n' "$dmg_path"
