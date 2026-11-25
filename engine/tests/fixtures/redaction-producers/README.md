# Redaction fixtures from real PDF producers

Real PDFs written by real producers, not files built in code. The gate
`scripts/verify-redaction-producers.mjs` redacts each one with the engine
built from this repo and checks the output with Poppler (`pdftotext`) and
pdf.js. CI runs it in the `redaction-producers` job against these committed
files; CI never regenerates them.

## Content

Every fixture holds the same lines on one page, each value on its own line:

| Line | Role |
|------|------|
| Claim Summary | KEEP (heading) |
| Policy 7781 | KEEP |
| Claimant | KEEP |
| Jane Doe | SENSITIVE (name) |
| SSN | KEEP |
| 123-45-6789 | SENSITIVE (SSN) |
| Email | KEEP |
| jane.doe@example.com | SENSITIVE (email) |
| Status Approved | KEEP |

The values are on separate lines on purpose. Forme removes a whole text run
when any of its glyphs is under a region (see `docs/concepts/redaction.mdx`,
Known Limitations), so a KEEP value in the same run as a SENSITIVE value
would be removed with it.

## Files

Generated 2026-11-24 on macOS 15.7.4 (24G517).

| File | Producer tool | `/Producer` | Fonts in the file | Bytes |
|------|---------------|-------------|-------------------|-------|
| `chrome.pdf` | Google Chrome 158.0.8127.37, headless print-to-PDF of `inputs/claim.html` | `Skia/PDF m158` | Helvetica, Helvetica-Bold: CID TrueType, Identity-H, ToUnicode | 19414 |
| `reportlab-helvetica.pdf` | ReportLab 4.4.4 (Python 3.9.12), `inputs/reportlab_claim.py` | `ReportLab PDF Library - www.reportlab.com` | Helvetica: standard 14 Type 1, WinAnsi, not embedded | 1573 |
| `reportlab-truetype.pdf` | ReportLab 4.4.4 (Python 3.9.12), `inputs/reportlab_claim.py` | `ReportLab PDF Library - www.reportlab.com` | IBM Plex Serif (`../fonts/IBMPlexSerif-Regular.ttf`): TrueType subset, ToUnicode | 14459 |
| `quartz-cupsfilter.pdf` | macOS CUPS text filter (`cgtexttopdf`) on `inputs/claim.txt` | `macOS Version 15.7.4 (Build 24G517) Quartz PDFContext` | Monaco: TrueType subset, MacRoman, no ToUnicode | 16738 |
| `quartz-coretext.pdf` | CoreText into a `CGPDFContext`, `inputs/quartz_claim.swift` (Swift 6.2.4) | `macOS Version 15.7.4 (Build 24G517) Quartz PDFContext` | Helvetica: TrueType subset, MacRoman, no ToUnicode | 15840 |

SHA-256:

```
d9dbd4baa80aa073fe8a831af73a1f142482016cf0baf401ffbbc558a8bd50d3  chrome.pdf
02b227eedbf318b18355d5c6e33fa2985caa3860725a644493e4f47f2687fd3c  reportlab-helvetica.pdf
070f15cdb61a0f76c879dc75994e673f4176cd38b337b0b778ffb19e47c37004  reportlab-truetype.pdf
292f884eaa75151679c51728d8abf55d9c44a8c178d6cb323c15409574779727  quartz-cupsfilter.pdf
553ccc3b5e3c296e235502747fdb240b50471cc9d1b6957147f61ddaa31df4b4  quartz-coretext.pdf
```

## Commands

`regenerate.sh` runs all of these (macOS only):

```sh
# Chrome (no --user-data-dir: Chrome hangs after printing with one)
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless --disable-gpu \
  --no-pdf-header-footer --print-to-pdf=chrome.pdf "file://$PWD/inputs/claim.html"

# ReportLab, in a throwaway venv outside the repo
python3 -m venv "$TMPDIR/rl" && "$TMPDIR/rl/bin/pip" install reportlab==4.4.4
"$TMPDIR/rl/bin/python" inputs/reportlab_claim.py . ../fonts/IBMPlexSerif-Regular.ttf

# macOS Quartz
cupsfilter -m application/pdf inputs/claim.txt > quartz-cupsfilter.pdf
swift inputs/quartz_claim.swift quartz-coretext.pdf
```

Chrome, cupsfilter and CoreText stamp the creation date into the file, so a
rerun gives new bytes with the same content. Check a regenerated file's
`/Producer` with `pdfinfo` and its text with `pdftotext` before committing it.

## Running the gate

```sh
# after building packages/core (npm run build, or build:ts over a downloaded WASM)
node scripts/verify-redaction-producers.mjs
OUT_DIR=/tmp/redacted node scripts/verify-redaction-producers.mjs   # keep the outputs
```

It needs `pdftotext` (poppler-utils) and fails without it.
