#!/usr/bin/env node

"use strict";

const { execFileSync } = require("child_process");
const path = require("path");
const fs = require("fs");
const crypto = require("crypto");

const PLATFORMS = {
  "darwin arm64": {
    pkg: "@context-keeper/mcp-darwin-arm64",
    tag: "darwin-arm64",
  },
  "darwin x64": {
    pkg: "@context-keeper/mcp-darwin-x64",
    tag: "darwin-x64",
  },
  "linux arm64": {
    pkg: "@context-keeper/mcp-linux-arm64",
    tag: "linux-arm64",
  },
  "linux x64": {
    pkg: "@context-keeper/mcp-linux-x64",
    tag: "linux-x64",
  },
};

// When truthy, the integrity check is skipped. Intended for debugging only —
// print a loud warning whenever set so the escape hatch cannot be used silently.
const SKIP_VERIFY_ENV = "CONTEXT_KEEPER_SKIP_INTEGRITY";

const key = `${process.platform} ${process.arch}`;
const entry = PLATFORMS[key];

if (!entry) {
  console.error(
    `context-keeper-mcp: unsupported platform ${process.platform} ${process.arch}\n` +
      `Supported: ${Object.keys(PLATFORMS).join(", ")}`
  );
  process.exit(1);
}

const { pkg, tag } = entry;

let binPath;
try {
  binPath = path.join(
    path.dirname(require.resolve(`${pkg}/package.json`)),
    "context-keeper-mcp"
  );
} catch {
  console.error(
    `context-keeper-mcp: could not find package ${pkg}.\n` +
      `This usually means the optional dependency was not installed.\n` +
      `Try: npm install context-keeper-mcp --force`
  );
  process.exit(1);
}

if (!fs.existsSync(binPath)) {
  console.error(
    `context-keeper-mcp: binary not found at ${binPath}\n` +
      `Try reinstalling: npm install context-keeper-mcp --force`
  );
  process.exit(1);
}

// ---------------------------------------------------------------------------
// SHA-256 integrity verification.
//
// The publish workflow ships a SHA256SUMS manifest inside this package with
// lines of the form "<hex>  <platform-tag>". We re-hash the resolved binary
// and fail hard on mismatch. No silent fallback — a mismatch could mean the
// platform package was tampered with after install, or a stale/corrupt cache.
// ---------------------------------------------------------------------------
function loadExpectedHash(manifestPath, platformTag) {
  const raw = fs.readFileSync(manifestPath, "utf8");
  for (const line of raw.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith("#")) continue;
    // Accept both "<hex>  <tag>" and "<hex> <tag>".
    const match = trimmed.match(/^([0-9a-fA-F]{64})\s+(.+)$/);
    if (!match) continue;
    const [, hex, name] = match;
    if (name.trim() === platformTag) {
      return hex.toLowerCase();
    }
  }
  return null;
}

function hashFileSync(filePath) {
  const hash = crypto.createHash("sha256");
  const fd = fs.openSync(filePath, "r");
  try {
    const buf = Buffer.allocUnsafe(64 * 1024);
    let bytesRead;
    // eslint-disable-next-line no-cond-assign
    while ((bytesRead = fs.readSync(fd, buf, 0, buf.length, null)) > 0) {
      hash.update(buf.subarray(0, bytesRead));
    }
  } finally {
    fs.closeSync(fd);
  }
  return hash.digest("hex");
}

function verifyIntegrity(binaryPath, platformTag) {
  if (process.env[SKIP_VERIFY_ENV]) {
    console.error(
      `context-keeper-mcp: WARNING — ${SKIP_VERIFY_ENV} is set; skipping ` +
        `SHA-256 integrity check. This is unsafe for anything other than ` +
        `local debugging.`
    );
    return;
  }

  const manifestPath = path.join(__dirname, "..", "SHA256SUMS");
  if (!fs.existsSync(manifestPath)) {
    // A released package MUST ship SHA256SUMS. If it's missing, we refuse to
    // run rather than silently skip — this catches repacking tampering where
    // an attacker could drop the manifest to bypass the check.
    console.error(
      `context-keeper-mcp: SHA256SUMS manifest missing at ${manifestPath}.\n` +
        `Refusing to run — published packages are expected to ship the\n` +
        `checksum manifest. Reinstall with: npm install context-keeper-mcp --force\n` +
        `If you are running from a local checkout, set ${SKIP_VERIFY_ENV}=1 to bypass.`
    );
    process.exit(1);
  }

  const expected = loadExpectedHash(manifestPath, platformTag);
  if (!expected) {
    console.error(
      `context-keeper-mcp: SHA256SUMS has no entry for platform "${platformTag}".\n` +
        `Manifest: ${manifestPath}\n` +
        `Refusing to run.`
    );
    process.exit(1);
  }

  const actual = hashFileSync(binaryPath).toLowerCase();
  if (actual !== expected) {
    console.error(
      `context-keeper-mcp: SHA-256 integrity check FAILED for ${binaryPath}\n` +
        `  platform: ${platformTag}\n` +
        `  expected: ${expected}\n` +
        `  actual:   ${actual}\n` +
        `The binary does not match the published checksum. This may indicate\n` +
        `tampering, a corrupted install, or a stale cache. Refusing to execute.\n` +
        `Reinstall with: npm install context-keeper-mcp --force`
    );
    process.exit(1);
  }
}

verifyIntegrity(binPath, tag);

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: "inherit" });
} catch (e) {
  if (e.status !== undefined) {
    process.exit(e.status);
  }
  throw e;
}
