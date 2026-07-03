#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

if [[ "$(uname -s)" != "Darwin" ]]; then
  printf 'macOS packaging must run on macOS; current OS is %s.\n' "$(uname -s)" >&2
  exit 1
fi

run_rust() {
  if command -v mise >/dev/null 2>&1; then
    mise exec rust@1.96.0 -- "$@"
  else
    "$@"
  fi
}

run_rust rustup target add aarch64-apple-darwin x86_64-apple-darwin
run_rust npm --workspace app run tauri -- build --target universal-apple-darwin --bundles app --ci --no-sign

bundle_root="target/universal-apple-darwin/release/bundle"
app_path="$bundle_root/macos/c4lens.app"
product_name="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").productName')"
version="$(node -p 'require("./crates/c4lens-tauri/tauri.conf.json").version')"
dmg_dir="$bundle_root/dmg"
stage_dir="$dmg_dir/stage"
dmg_path="$dmg_dir/${product_name}_${version}_universal.dmg"

if [[ ! -d "$app_path" ]]; then
  printf 'App bundle not found after Tauri build: %s\n' "$app_path" >&2
  exit 1
fi

rm -f "$bundle_root"/macos/rw.*.dmg "$bundle_root"/macos/*.dmg
rm -rf "$stage_dir"
mkdir -p "$stage_dir" "$dmg_dir"
cp -R "$app_path" "$stage_dir/"
ln -s /Applications "$stage_dir/Applications"

rm -f "$dmg_path"
hdiutil create -volname "$product_name" -srcfolder "$stage_dir" -ov -format UDZO "$dmg_path"
rm -rf "$stage_dir"

printf 'Built unsigned macOS artifacts under %s\n' "$bundle_root"
