#!/usr/bin/env node
// Dry-run test for the wrapper's SHA-256 integrity check.
//
// Run with: node npm/context-keeper-mcp/test/verify.test.js
//
// The test stages a fake platform package (with a tiny shim binary) under a
// sandbox node_modules tree, points a SHA256SUMS manifest at it, and then
// invokes the real bin/run.js wrapper. It asserts that:
//   1. A pristine binary whose hash matches the manifest is accepted.
//   2. A one-byte-tampered binary is rejected with a clear error listing
//      both the expected and actual hashes.
//   3. A missing SHA256SUMS manifest causes the wrapper to refuse to run.
//
// This is the "locally-tampered binary is rejected" acceptance check.

"use strict";

const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const { execFileSync } = require("child_process");
const os = require("os");

const wrapperRoot = path.resolve(__dirname, "..");
const wrapperEntry = path.join(wrapperRoot, "bin/run.js");

const PLATFORM_MAP = {
  "darwin arm64": "mcp-darwin-arm64",
  "darwin x64": "mcp-darwin-x64",
  "linux arm64": "mcp-linux-arm64",
  "linux x64": "mcp-linux-x64",
};
const platformKey = `${process.platform} ${process.arch}`;
const platformName = PLATFORM_MAP[platformKey];
if (!platformName) {
  console.log(`SKIP: unsupported test platform ${platformKey}`);
  process.exit(0);
}
const platformTag = platformName.replace(/^mcp-/, "");

function sha256OfFile(p) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(p));
  return hash.digest("hex");
}

const sandbox = fs.mkdtempSync(path.join(os.tmpdir(), "ck-wrapper-test-"));
process.on("exit", () => {
  try { fs.rmSync(sandbox, { recursive: true, force: true }); } catch {}
});

// Set up a sandbox node_modules tree with a fake platform package.
const platDir = path.join(sandbox, "node_modules/@context-keeper", platformName);
fs.mkdirSync(platDir, { recursive: true });
fs.writeFileSync(
  path.join(platDir, "package.json"),
  JSON.stringify({ name: `@context-keeper/${platformName}`, version: "0.0.0-test" }, null, 2)
);

// Fake binary — something short and printable. We invoke the wrapper with a
// harmless argument that exits 0 so the happy-path test can distinguish
// "binary ran" from "integrity check failed".
const binShim = "#!/bin/sh\nexit 0\n";
const binPath = path.join(platDir, "context-keeper-mcp");
fs.writeFileSync(binPath, binShim, { mode: 0o755 });

// Copy the wrapper into the sandbox so its node_modules resolution picks up
// our fake platform package, not any real install.
const wrapperCopy = path.join(sandbox, "wrapper");
fs.mkdirSync(wrapperCopy, { recursive: true });
fs.cpSync(wrapperRoot, wrapperCopy, {
  recursive: true,
  filter: (src) => !src.includes("node_modules") && !src.endsWith(".test.js"),
});

// Link the fake platform package into the wrapper's node_modules.
const wrapperNm = path.join(wrapperCopy, "node_modules/@context-keeper", platformName);
fs.mkdirSync(path.dirname(wrapperNm), { recursive: true });
fs.symlinkSync(platDir, wrapperNm);

const pristineHash = sha256OfFile(binPath);
const manifestPath = path.join(wrapperCopy, "SHA256SUMS");

function writeManifest(hex, tag = platformTag) {
  fs.writeFileSync(manifestPath, `${hex}  ${tag}\n`);
}

function runWrapper() {
  return execFileSync("node", [path.join(wrapperCopy, "bin/run.js")], {
    stdio: ["ignore", "pipe", "pipe"],
  });
}

let passed = 0, failed = 0;
function assert(cond, msg) {
  if (cond) { console.log("PASS:", msg); passed++; }
  else       { console.error("FAIL:", msg); failed++; }
}

// ---- Test 1: pristine binary with matching hash is accepted ---------------
writeManifest(pristineHash);
let happyOk = false;
try { runWrapper(); happyOk = true; } catch (e) {
  console.error("unexpected error on happy path:", e.stderr && e.stderr.toString());
}
assert(happyOk, "pristine binary with matching hash is executed");

// ---- Test 2: flip one byte in the binary → wrapper rejects ----------------
const buf = fs.readFileSync(binPath);
buf[buf.length - 1] = buf[buf.length - 1] ^ 0x01; // flip low bit of last byte
fs.writeFileSync(binPath, buf, { mode: 0o755 });
const tamperedHash = sha256OfFile(binPath);

// Manifest still advertises the pristine hash.
writeManifest(pristineHash);

let rejected = false, stderrText = "";
try { runWrapper(); } catch (e) {
  rejected = e.status === 1;
  stderrText = (e.stderr || "").toString();
}
assert(rejected, "tampered binary causes wrapper to exit with status 1");
assert(stderrText.includes("SHA-256 integrity check FAILED"), "error mentions integrity failure");
assert(stderrText.includes(pristineHash), "error prints expected hash");
assert(stderrText.includes(tamperedHash), "error prints actual hash");
assert(pristineHash !== tamperedHash, "tampered hash differs from pristine hash");

// ---- Test 3: missing manifest → wrapper refuses ---------------------------
fs.unlinkSync(manifestPath);
let refused = false, missingStderr = "";
try { runWrapper(); } catch (e) {
  refused = e.status === 1;
  missingStderr = (e.stderr || "").toString();
}
assert(refused, "missing manifest causes wrapper to exit with status 1");
assert(missingStderr.includes("SHA256SUMS manifest missing"), "error mentions missing manifest");

// ---- Summary --------------------------------------------------------------
console.log(`\n${passed} passed, ${failed} failed`);
process.exit(failed === 0 ? 0 : 1);
