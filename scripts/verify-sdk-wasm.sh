#!/usr/bin/env bash
# Verify the SDK-embedded WASM binaries match the CURRENT engine source.
#
# The Python and Go SDKs embed `forme.wasm` rather than linking the engine, so
# they ship whatever engine that file was built from regardless of what the
# monorepo released. A release that skips the rebuild ships the previous engine
# from both SDKs while npm and crates.io move.
#
# Confirming a rebuild COMMIT exists does not establish this. At 0.24.0 the Go
# SDK carried a commit titled "Rebuild embedded engine WASM for 0.24.0", made a
# day before the release, from an engine missing two of that release's five
# document-moving changes. Same message, same file present, `go test` green
# either way. A rebuild done too early is indistinguishable from a rebuild done
# right unless you compare the bytes.
#
# So this builds the wasm fresh from the current source and compares hashes. It
# never trusts an existing artifact, for the same reason scripts/byte-wall.sh
# does not: the stale-binary class has cost this project many hours.
#
# Usage:
#   scripts/verify-sdk-wasm.sh          # verify only; exit 1 on mismatch
#   scripts/verify-sdk-wasm.sh --fix    # verify, and update both copies if stale
set -uo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
GO_SDK="$HERE/../forme-go"
FIX=0
[ "${1:-}" = "--fix" ] && FIX=1

PY_WASM="$HERE/packages/python-sdk/formepdf/forme.wasm"
GO_WASM="$GO_SDK/templates/forme.wasm"

echo "── building forme.wasm from current engine + html source"
BUILD_DIR="$(mktemp -d /tmp/verify-sdk-wasm.XXXXXX)"
trap 'rm -rf "$BUILD_DIR"' EXIT

# Same target, features and profile overrides as packages/python-sdk/build_wasm.sh.
# If that script changes, this must change with it; they are the same build.
cargo build \
  --manifest-path "$HERE/html/Cargo.toml" \
  --lib \
  --target wasm32-wasip1 \
  --release \
  --features wasm-raw \
  --config profile.release.debug=false \
  --config profile.release.strip=true \
  --quiet || { echo "FAIL: wasm build failed"; exit 2; }

FRESH="$HERE/html/target/wasm32-wasip1/release/forme_pdf_html.wasm"
[ -f "$FRESH" ] || { echo "FAIL: build produced no artifact at $FRESH"; exit 2; }
cp "$FRESH" "$BUILD_DIR/fresh.wasm"
WANT="$(shasum -a 256 "$BUILD_DIR/fresh.wasm" | cut -d' ' -f1)"
echo "   engine source hashes to ${WANT:0:12}"

status=0
check() {
  local label="$1" path="$2"
  if [ ! -f "$path" ]; then
    echo "   MISSING  $label ($path)"
    status=1
    return
  fi
  local got
  got="$(shasum -a 256 "$path" | cut -d' ' -f1)"
  if [ "$got" = "$WANT" ]; then
    echo "   ok       $label"
  else
    echo "   STALE    $label — has ${got:0:12}, engine is ${WANT:0:12}"
    status=1
    if [ "$FIX" = "1" ]; then
      cp "$BUILD_DIR/fresh.wasm" "$path"
      echo "            rebuilt; commit it (python-sdk needs 'git add -f')"
    fi
  fi
}

check "python-sdk formepdf/forme.wasm" "$PY_WASM"
if [ -d "$GO_SDK" ]; then
  check "forme-go templates/forme.wasm" "$GO_WASM"
else
  echo "   SKIPPED  forme-go not checked out beside this repo ($GO_SDK)"
  echo "            its embedded wasm is UNVERIFIED — clone it and re-run"
  status=1
fi

if [ "$status" = "0" ]; then
  echo "── both SDK binaries match the current engine"
else
  [ "$FIX" = "1" ] && echo "── stale copies updated; re-run without --fix to confirm" \
                   || echo "── re-run with --fix to update, then commit in each repo"
fi
exit $status
