#!/bin/bash
# Regenerate the producer fixtures in this directory. macOS only: it needs
# Google Chrome, python3 (for a throwaway ReportLab venv), swift and
# pdffonts. CI never runs this; CI uses the committed PDFs.
#
# Usage: engine/tests/fixtures/redaction-producers/regenerate.sh
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
IN="$HERE/inputs"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
TTF="$HERE/../fonts/IBMPlexSerif-Regular.ttf"

SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/forme-redaction-producers.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

# Chrome: headless print-to-PDF. No --user-data-dir (Chrome hangs after
# printing with one).
"$CHROME" --headless --disable-gpu --no-pdf-header-footer \
  --print-to-pdf="$HERE/chrome.pdf" "file://$IN/claim.html"

# ReportLab: a throwaway venv outside the repo.
python3 -m venv "$SCRATCH/venv"
"$SCRATCH/venv/bin/pip" install -q "reportlab==${REPORTLAB_VERSION:-4.4.4}"
"$SCRATCH/venv/bin/python" "$IN/reportlab_claim.py" "$HERE" "$TTF"

# macOS Quartz: CoreText drawn into a CGPDFContext. (Not cupsfilter: its
# text filter, cgtexttopdf, hard-codes Apple's Monaco, and Apple font data
# must not be committed to this MIT repo.)
swift "$IN/quartz_claim.swift" "$HERE/quartz-coretext.pdf" "$TTF"

# Every embedded font must be IBM Plex (OFL). Fail on anything else
# (ReportLab's standard Helvetica is referenced, not embedded: allowed).
for f in "$HERE"/*.pdf; do
  # Columns from the end: emb sub uni object-id generation.
  if pdffonts "$f" | awk 'NR>2 && $(NF-4)=="yes" && $1 !~ /IBMPlex/' | grep .; then
    echo "$(basename "$f") embeds a font other than IBM Plex" >&2
    exit 1
  fi
done

for f in "$HERE"/*.pdf; do
  printf '%-28s %8s bytes  %s\n' "$(basename "$f")" "$(wc -c < "$f" | tr -d ' ')" \
    "$(pdfinfo "$f" 2>/dev/null | sed -n 's/^Producer: *//p')"
done
