"use strict";

const { spawn } = require("node:child_process");
const path = require("node:path");

const targets = {
  "darwin-arm64": ["darwin-arm64", "ig"],
  "darwin-x64": ["darwin-x64", "ig"],
  "linux-arm64": ["linux-arm64", "ig"],
  "linux-x64": ["linux-x64", "ig"],
  "win32-x64": ["win32-x64", "ig.exe"],
};

const key = `${process.platform}-${process.arch}`;
const target = targets[key];
if (!target) {
  process.stderr.write(`ivygrep MCP bundle does not support ${key}\n`);
  process.exit(1);
}

const executable = path.join(__dirname, "bin", ...target);
const child = spawn(executable, ["--mcp"], {
  stdio: "inherit",
  windowsHide: true,
});

child.on("error", (error) => {
  process.stderr.write(`failed to start ivygrep MCP server: ${error.message}\n`);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => child.kill(signal));
}
