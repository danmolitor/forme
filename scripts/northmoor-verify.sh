#!/usr/bin/env bash
# Per-template verification for the Northmoor set. Usage:
#   bash scripts/northmoor-verify.sh <template-name> [expected-pages]
# Renders via the release CLI (build it fresh first), reports warnings
# verbatim, page count, PDF/UA-1 via veraPDF, and the content audit.
set -uo pipefail
T="${1:?template name}"
EXPECT="${2:-}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/html/target/release/forme-html"
F="$ROOT/packages/fonts-standard/fonts"
SRC="$ROOT/templates/$T/index.html"
OUT="/tmp/northmoor-$T.pdf"
[ -f "$SRC" ] || { echo "FAIL: $SRC missing"; exit 1; }

echo "── $T"
WARN=$("$BIN" "$SRC" -o "$OUT" --audit-content 2>&1 >/dev/null | grep -v "^$OUT" || true)
if [ -n "$WARN" ]; then echo "WARNINGS (must be empty):"; echo "$WARN"; else echo "warnings: none"; fi

PAGES=$(python3 -c "import re;print(len(re.findall(rb'/Type /Page[^s]',open('$OUT','rb').read())))")
if [ -n "$EXPECT" ]; then
  [ "$PAGES" = "$EXPECT" ] && echo "pages: $PAGES (matches README)" || echo "pages: $PAGES — EXPECTED $EXPECT"
else
  echo "pages: $PAGES"
fi

"$BIN" "$SRC" -o "$OUT.ua.pdf" --pdf-ua --lang en \
  --font "Helvetica=$F/LiberationSans-Regular.ttf" \
  --font "Helvetica:bold=$F/LiberationSans-Bold.ttf" \
  --font "Helvetica:italic=$F/LiberationSans-Italic.ttf" \
  --font "Helvetica:bold:italic=$F/LiberationSans-BoldItalic.ttf" \
  --font "Georgia=$F/LiberationSerif-Regular.ttf" \
  --font "Georgia:bold=$F/LiberationSerif-Bold.ttf" \
  --font "Georgia:italic=$F/LiberationSerif-Italic.ttf" \
  --font "Menlo=$F/LiberationMono-Regular.ttf" \
  >/dev/null 2>/tmp/northmoor-ua-warn.txt
UAWARN=$(grep -v "^$" /tmp/northmoor-ua-warn.txt || true)
[ -n "$UAWARN" ] && { echo "UA-render warnings:"; echo "$UAWARN"; }
UA=$("$HOME/verapdf/verapdf" --flavour ua1 "$OUT.ua.pdf" 2>/dev/null | grep -o 'isCompliant="[a-z]*"' | head -1)
echo "PDF/UA-1: $UA"
[ "$UA" = 'isCompliant="true"' ] || echo "FAIL: not UA-1 compliant"
