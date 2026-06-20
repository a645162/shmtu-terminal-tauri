#!/usr/bin/env node

import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, "../..");
const tauriCli = path.join(projectRoot, "node_modules", "@tauri-apps", "cli", "tauri.js");

const env = { ...process.env };

// linuxdeploy's AppImage currently requires libfuse.so.2 at runtime.
// On Ubuntu 24.04, forcing extract-and-run avoids that host dependency.
if (process.platform === "linux" && !env.APPIMAGE_EXTRACT_AND_RUN) {
  env.APPIMAGE_EXTRACT_AND_RUN = "1";
}

const child = spawn(process.execPath, [tauriCli, ...process.argv.slice(2)], {
  cwd: projectRoot,
  env,
  stdio: "inherit",
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 1);
});

child.on("error", (error) => {
  console.error(error);
  process.exit(1);
});
