#!/bin/bash
set -e
cd "$(dirname "$0")/../../engine"
wasm-pack build --target web --out-dir ../packages/core/pkg --features wasm
