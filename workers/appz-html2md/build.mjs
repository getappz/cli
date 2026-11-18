// Builds the shared Rust converter to wasm (size/import gates included) and
// copies it next to the worker source.
import { execFileSync } from "node:child_process";
import { copyFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..");
execFileSync("bash", [join(root, "scripts", "check-html2md-wasm.sh")], { cwd: root, stdio: "inherit" });
copyFileSync(
  join(root, "target", "wasm32-unknown-unknown", "wasm", "appz_html2md.wasm"),
  join(here, "src", "html2md.wasm"),
);
