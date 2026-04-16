#!/usr/bin/env node

"use strict";

const { execFileSync } = require("child_process");
const path = require("path");
const fs = require("fs");

const PLATFORMS = {
  "darwin arm64": "@context-keeper/mcp-darwin-arm64",
  "darwin x64": "@context-keeper/mcp-darwin-x64",
  "linux arm64": "@context-keeper/mcp-linux-arm64",
  "linux x64": "@context-keeper/mcp-linux-x64",
};

const key = `${process.platform} ${process.arch}`;
const pkg = PLATFORMS[key];

if (!pkg) {
  console.error(
    `context-keeper-mcp: unsupported platform ${process.platform} ${process.arch}\n` +
      `Supported: ${Object.keys(PLATFORMS).join(", ")}`
  );
  process.exit(1);
}

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

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: "inherit" });
} catch (e) {
  if (e.status !== undefined) {
    process.exit(e.status);
  }
  throw e;
}
