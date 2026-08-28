#!/bin/bash
# Build the HTML input path to WASM for Node (the npx / server use case).
# Browser and edge targets follow the same pattern as packages/core when
# they ship.
set -e
cd "$(dirname "$0")/../../html"
wasm-pack build --target nodejs --out-dir ../packages/html/pkg-node -- --features wasm

# wasm-pack writes a catch-all .gitignore into the out dir, and npm honors
# it even for "files"-listed directories — the tarball silently ships
# without the WASM. Remove it.
rm -f ../packages/html/pkg-node/.gitignore
