#!/usr/bin/env bash
# Build the Forme engine as a WASI WASM module for use with Python wasmtime.
# Output: formepdf/forme.wasm
#
# Built from the `html` crate (not `engine`): the html crate depends on the
# engine, so one wasm carries BOTH the HTML input path (forme_render_html) and
# the engine surface (forme_render_pdf, forme_certify_pdf, …). Its wasm_raw.rs
# is the single C-ABI; the engine's own wasm-raw is left disabled here, so there
# are no duplicate symbols (that one still serves the engine-only Go/wazero wasm).
#
# `--config profile.release.debug=false` overrides the crate's release profile
# (which carries debug info for the dhat examples) so the shipped wasm is ~6.5MB,
# not ~40MB. `strip=true` drops the rest.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HTML_DIR="$SCRIPT_DIR/../../html"

echo "Building Forme WASM (wasm32-wasip1, release, from the html crate)..."
cargo build \
  --manifest-path "$HTML_DIR/Cargo.toml" \
  --lib \
  --target wasm32-wasip1 \
  --release \
  --features wasm-raw \
  --config profile.release.debug=false \
  --config profile.release.strip=true

WASM_SRC="$HTML_DIR/target/wasm32-wasip1/release/forme_pdf_html.wasm"
WASM_DST="$SCRIPT_DIR/formepdf/forme.wasm"

cp "$WASM_SRC" "$WASM_DST"
echo "Copied to $WASM_DST ($(du -h "$WASM_DST" | cut -f1))"
