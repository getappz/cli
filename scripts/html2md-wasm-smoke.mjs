// Gate for the appz-html2md wasm: no imports, and a tiny page converts.
// Usage: node html2md-wasm-smoke.mjs <path-to.wasm>
import { readFileSync } from "node:fs";

const bytes = readFileSync(process.argv[2]);
const module = new WebAssembly.Module(bytes);

const imports = WebAssembly.Module.imports(module);
if (imports.length > 0) {
  console.error("FAIL: wasm has imports:", JSON.stringify(imports));
  process.exit(1);
}

const { exports } = new WebAssembly.Instance(module, {});
for (const name of ["memory", "md_alloc", "md_free", "md_convert"]) {
  if (!(name in exports)) {
    console.error(`FAIL: missing export ${name}`);
    process.exit(1);
  }
}

function put(text) {
  const data = new TextEncoder().encode(text);
  const ptr = exports.md_alloc(data.length);
  new Uint8Array(exports.memory.buffer, ptr, data.length).set(data);
  return { ptr, len: data.length };
}

const html = put("<html><body><main><h1>Hello agents</h1><p>Body text.</p></main></body></html>");
const origin = put("https://example.com/");
const out = exports.md_convert(html.ptr, html.len, origin.ptr, origin.len);
exports.md_free(html.ptr, html.len);
exports.md_free(origin.ptr, origin.len);
if (out === 0) {
  console.error("FAIL: md_convert returned 0");
  process.exit(1);
}

const view = new DataView(exports.memory.buffer, out, 9);
const total = view.getUint32(0, true);
const md = new TextDecoder().decode(new Uint8Array(exports.memory.buffer, out + 9, total - 9));
exports.md_free(out, total);
if (!md.includes("# Hello agents")) {
  console.error("FAIL: unexpected markdown:", JSON.stringify(md));
  process.exit(1);
}
console.log("smoke ok:", JSON.stringify(md.slice(0, 40)));
