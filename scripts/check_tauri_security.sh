#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

node <<'NODE'
const fs = require("node:fs");

const capability = JSON.parse(fs.readFileSync("crates/c4lens-tauri/capabilities/default.json", "utf8"));
const config = JSON.parse(fs.readFileSync("crates/c4lens-tauri/tauri.conf.json", "utf8"));

const expectedPermissions = ["core:default", "dialog:allow-open"];
const permissions = [...capability.permissions].sort();

assertArrayEqual(permissions, expectedPermissions, "default capability permissions");
assertArrayEqual(capability.windows, ["main"], "default capability windows");

const deniedPermissionPrefixes = ["fs:", "path:", "process:", "shell:", "sql:", "window:"];
for (const permission of permissions) {
  const deniedPrefix = deniedPermissionPrefixes.find((prefix) => permission.startsWith(prefix));
  if (deniedPrefix) {
    throw new Error(`default capability must not include ${deniedPrefix} permission: ${permission}`);
  }
}

const csp = config.app?.security?.csp;
if (!csp) {
  throw new Error("production CSP is missing");
}

assertArrayEqual(csp["default-src"], ["'self'", "ipc:"], "default-src");
assertArrayEqual(csp["script-src"], ["'self'"], "script-src");
assertArrayEqual(csp["style-src"], ["'self'", "'unsafe-inline'"], "style-src");
assertArrayEqual(csp["img-src"], ["'self'", "asset:", "data:"], "img-src");
assertArrayEqual(csp["font-src"], ["'self'", "data:"], "font-src");
assertArrayEqual(csp["connect-src"], ["'self'", "ipc:"], "connect-src");
assertArrayEqual(csp["object-src"], ["'none'"], "object-src");
assertArrayEqual(csp["base-uri"], ["'none'"], "base-uri");
assertArrayEqual(csp["frame-ancestors"], ["'none'"], "frame-ancestors");

for (const [directive, values] of Object.entries(csp)) {
  for (const value of values) {
    if (/^https?:|^ws:|^wss:/.test(value)) {
      throw new Error(`production CSP ${directive} must not include network source: ${value}`);
    }
  }
}

console.log("Verified Tauri capability and production CSP.");

function assertArrayEqual(actual, expected, label) {
  const actualJson = JSON.stringify(actual);
  const expectedJson = JSON.stringify(expected);
  if (actualJson !== expectedJson) {
    throw new Error(`${label} mismatch: expected ${expectedJson}, got ${actualJson}`);
  }
}
NODE
