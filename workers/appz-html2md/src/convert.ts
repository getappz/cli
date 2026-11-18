// Thin host for the Rust converter's C-ABI (crates/appz-html2md/src/wasm.rs).
import wasmModule from "./html2md.wasm";

export const MAX_HTML_BYTES = 2 * 1024 * 1024;

export interface Converted {
  markdown: string;
  tokens: number;
  mode: "minimal" | "fallback";
}

interface Exports {
  memory: WebAssembly.Memory;
  md_alloc(len: number): number;
  md_free(ptr: number, len: number): void;
  md_convert(htmlPtr: number, htmlLen: number, originPtr: number, originLen: number): number;
}

/** How many times the module was instantiated in this isolate (tests assert 1). */
export let instantiations = 0;

let instance: Promise<Exports> | undefined;

function getInstance(): Promise<Exports> {
  instance ??= WebAssembly.instantiate(wasmModule, {}).then((i) => {
    instantiations++;
    return i.exports as unknown as Exports;
  });
  return instance;
}

export async function convertHtml(html: ArrayBuffer, origin: string): Promise<Converted | null> {
  if (html.byteLength > MAX_HTML_BYTES) return null;
  const wasm = await getInstance();
  const originBytes = new TextEncoder().encode(origin);
  const htmlBytes = new Uint8Array(html);
  const htmlPtr = wasm.md_alloc(htmlBytes.length);
  const originPtr = wasm.md_alloc(originBytes.length);
  let outPtr = 0;
  let outLen = 0;
  try {
    // Memory may grow during md_alloc: take views only after both allocations.
    new Uint8Array(wasm.memory.buffer, htmlPtr, htmlBytes.length).set(htmlBytes);
    new Uint8Array(wasm.memory.buffer, originPtr, originBytes.length).set(originBytes);
    outPtr = wasm.md_convert(htmlPtr, htmlBytes.length, originPtr, originBytes.length);
    if (outPtr === 0) return null;
    // Frame: [u32 total_len][u32 tokens][u8 mode][markdown...], all LE.
    const head = new DataView(wasm.memory.buffer, outPtr, 9);
    outLen = head.getUint32(0, true);
    const tokens = head.getUint32(4, true);
    const mode = head.getUint8(8) === 1 ? "fallback" : "minimal";
    const markdown = new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, outPtr + 9, outLen - 9));
    return { markdown, tokens, mode };
  } finally {
    wasm.md_free(htmlPtr, htmlBytes.length);
    wasm.md_free(originPtr, originBytes.length);
    if (outPtr !== 0) wasm.md_free(outPtr, outLen);
  }
}
