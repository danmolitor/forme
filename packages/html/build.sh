#!/bin/bash
# Build the HTML input path to WASM for three consumer shapes, mirroring
# packages/core:
#
#   pkg/      — wasm-pack --target bundler — Vite, Webpack, Turbopack,
#               esbuild. WASM instantiated implicitly at module load.
#   pkg-web/  — wasm-pack --target web     — Cloudflare Workers and other
#               edge runtimes; driven by an explicit init(module) call.
#   pkg-node/ — wasm-pack --target nodejs  — Node / npx; self-initializes
#               via fs.readFileSync at require time.
set -e
cd "$(dirname "$0")/../../html"
wasm-pack build --target bundler --out-dir ../packages/html/pkg      -- --features wasm
wasm-pack build --target web     --out-dir ../packages/html/pkg-web  -- --features wasm
wasm-pack build --target nodejs  --out-dir ../packages/html/pkg-node -- --features wasm

# wasm-pack writes a catch-all .gitignore into EVERY out dir, and npm honors
# it even for "files"-listed directories — the tarball silently ships
# without the WASM. Remove it from all three (this trap already bit once:
# a 5-file tarball with no WASM in it).
rm -f ../packages/html/pkg/.gitignore \
      ../packages/html/pkg-web/.gitignore \
      ../packages/html/pkg-node/.gitignore
