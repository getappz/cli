import { env } from "cloudflare:workers";
import { expect, it } from "vitest";
import { Html2Md } from "../src/index";
import { MAX_HTML_BYTES, instantiations } from "../src/convert";

const enc = (s: string) => new TextEncoder().encode(s).buffer as ArrayBuffer;
const worker = () => new Html2Md({} as any, env as any);
const PAGE = "<html><body><main><h1>Hello agents</h1><p>Body text.</p></main></body></html>";

it("converts html to markdown", async () => {
  const out = await worker().convert(enc(PAGE), "https://example.com/");
  expect(out).not.toBeNull();
  expect(out!.markdown).toContain("# Hello agents");
  expect(out!.mode).toBe("minimal");
  expect(out!.tokens).toBeGreaterThan(0);
});

it("returns null over the 2 MB cap", async () => {
  const big = new ArrayBuffer(MAX_HTML_BYTES + 1);
  expect(await worker().convert(big, "https://example.com/")).toBeNull();
});

it("returns null for an empty result", async () => {
  expect(await worker().convert(enc("<html><body></body></html>"), "https://example.com/")).toBeNull();
});

it("reuses one wasm instance across 3 calls", async () => {
  for (let i = 0; i < 3; i++) await worker().convert(enc(PAGE), "https://example.com/");
  expect(instantiations).toBe(1);
});

it("survives invalid origin", async () => {
  expect(await worker().convert(enc(PAGE), "not a url")).toBeNull();
});
