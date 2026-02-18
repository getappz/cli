#!/usr/bin/env node
"use strict";

// Launches `appz mcp`. Requires appz to be installed (cargo install appz, or from release).
const { spawn } = require("node:child_process");

function main() {
  const child = spawn("appz", ["mcp", ...process.argv.slice(2)], {
    stdio: "inherit",
  });

  const forwardSignal = (signal) => {
    if (!child.killed) {
      try {
        child.kill(signal);
      } catch {
        /* ignore */
      }
    }
  };

  ["SIGINT", "SIGTERM", "SIGHUP"].forEach((sig) => {
    process.on(sig, () => forwardSignal(sig));
  });

  child.on("error", (err) => {
    console.error("Failed to start appz mcp:", err.message);
    console.error(
      "Ensure appz is installed: cargo install appz, or install from release."
    );
    process.exit(1);
  });

  child.on("exit", (code, signal) => {
    if (signal) {
      try {
        process.kill(process.pid, signal);
      } catch {
        process.exit(1);
      }
    } else {
      process.exit(code ?? 1);
    }
  });
}

main();
