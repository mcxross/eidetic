#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const packageRoot = path.resolve(__dirname, "..");

const BINARY_NAME = process.platform === "win32" ? "eidetic.exe" : "eidetic";
const binPath = path.join(packageRoot, "native", BINARY_NAME);

if (!fs.existsSync(binPath)) {
  console.error(`[eidetic-mcp] Error: Native binary not found at ${binPath}`);
  console.error("[eidetic-mcp] It seems the postinstall script did not successfully download the binary.");
  console.error("[eidetic-mcp] Please check your internet connection or run npm install again.");
  process.exit(1);
}

// Spawn the native binary, passing all arguments and passing through standard I/O transparently.
// stdio: "inherit" is crucial for MCP's JSON-RPC over stdio to work.
const result = spawnSync(binPath, process.argv.slice(2), {
  stdio: "inherit",
});

if (result.error) {
  console.error(`[eidetic-mcp] Failed to start native binary: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 0);
