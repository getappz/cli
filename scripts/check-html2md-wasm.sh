#!/usr/bin/env bash
# Build the appz-html2md wasm module and gate it: <= 150 KB gzipped, zero
# imports, and a node smoke test through the C-ABI. Prints the artifact path
# on success (workers/appz-html2md/build.mjs copies it).
set -euo pipefail

MAX_GZ_BYTES=$((150 * 1024))
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

cargo build -j "${CARGO_BUILD_JOBS:-6}" --profile wasm --target wasm32-unknown-unknown -p appz-html2md --no-default-features
WASM="target/wasm32-unknown-unknown/wasm/appz_html2md.wasm"
[ -f "$WASM" ] || { echo "FAIL: $WASM missing" >&2; exit 1; }

RAW=$(wc -c <"$WASM")
GZ=$(gzip -9 -c "$WASM" | wc -c)
echo "wasm: raw=${RAW}B gzip=${GZ}B (limit ${MAX_GZ_BYTES}B)"
if [ "$GZ" -gt "$MAX_GZ_BYTES" ]; then
  echo "FAIL: gzipped wasm is ${GZ}B, over ${MAX_GZ_BYTES}B" >&2
  exit 1
fi

node "$ROOT/scripts/html2md-wasm-smoke.mjs" "$WASM"
echo "OK $WASM"
