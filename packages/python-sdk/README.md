# formepdf

Python SDK for [Forme](https://formepdf.com) — a page-native PDF rendering engine.
Build documents from a component DSL and render them to PDF **locally**, in-process,
with **no browser, no headless Chromium, and no system libraries** — the engine ships
as a single WebAssembly module and runs anywhere Python runs (including serverless).

- **Local rendering** via `wasmtime` — no API key, no network calls.
- **Real print features** — flexbox layout, tables with headers that repeat across
  pages, fixed headers/footers, page breaks, watermarks.
- **Accessible & archival** — tagged PDF, **PDF/UA-1**, and **PDF/A** produced locally,
  the same conformance the engine gives everywhere.
- **Charts, QR codes, barcodes, and fillable form fields** built in.
- **Local digital signing** (PKCS#7 / X.509).

## Installation

```bash
pip install formepdf[local]
```

The `[local]` extra adds `wasmtime`; the engine WASM is bundled in the wheel. That's
all you need — no cairo, no pango, no fonts to install.

## Quick start

```python
from formepdf import Document, Page, View, Text

doc = Document(
    Page(
        View(
            Text("Invoice #001", font_size=24, font_weight="bold"),
            Text("Acme Corp", font_size=14, color="#666"),
            flex_direction="column", gap=8,
        ),
    ),
    title="Invoice #001",
)

pdf = doc.render()               # -> bytes

with open("invoice.pdf", "wb") as f:
    f.write(pdf)
```

## Accessible & archival output

The conformance options live on `Document` — no extra pipeline, no external validator
step to produce them:

```python
doc = Document(
    Page(Text("Annual Report 2026", font_size=20)),
    title="Annual Report 2026",
    lang="en",
    tagged=True,     # structure tree (BDC/EMC marked content)
    pdf_ua=True,     # PDF/UA-1 (accessibility)
    pdfa="2b",       # PDF/A-2b (archival); also "2u", "2a", "3b", "3u", "3a"
)
pdf = doc.render()
```

`Document` options: `title`, `author`, `subject`, `lang`, `tagged`, `pdf_ua`, `pdfa`,
`flatten_forms`.

## Components

| Component | Description |
|-----------|-------------|
| `Document(*children)` | Root container. `.render()` returns PDF bytes. Options: `title`, `author`, `subject`, `lang`, `tagged`, `pdf_ua`, `pdfa`, `flatten_forms` |
| `Page(*children)` | Page container. Options: `size` (e.g. `"A4"`, `"Letter"`), `margin` |
| `View(*children)` | Flex/grid container. Options: all style kwargs (`flex_direction`, `gap`, `padding`, etc.) |
| `Text(content)` | Text element. Options: `font_size`, `font_weight`, `color`, `text_align`, etc. |
| `Image(src)` | Image (file path, URL, or data URI). Options: `width`, `height`, `alt` |
| `Table(*rows)` | Table with auto-repeating headers. Options: `columns` |
| `Row(*cells)` | Table row. Options: `header=True` for repeat-on-page-break |
| `Cell(*children)` | Table cell. Options: `col_span`, `row_span` |
| `Svg(content)` | Inline SVG. Options: `width`, `height` |
| `QrCode(data)` | Vector QR code. Options: `size`, `color` |
| `Barcode(data)` | 1D barcode. Options: `format` (`"Code128"`, `"Code39"`, `"EAN13"`, etc.), `width`, `height` |
| `BarChart(data)` | Bar chart. Options: `width`, `height`, `color`, `title` |
| `LineChart(series, labels)` | Line chart. Options: `width`, `height`, `show_points`, `title` |
| `PieChart(data)` | Pie/donut chart. Options: `width`, `height`, `donut`, `title` |
| `AreaChart(series, labels)` | Area chart. Options: `width`, `height`, `title` |
| `DotPlot(groups)` | Scatter plot. Options: `width`, `height`, `title` |
| `TextField(name)` | Fillable text field. Options: `value`, `placeholder`, `multiline` |
| `Checkbox(name)` | Fillable checkbox. Options: `checked` |
| `Dropdown(name, options)` | Fillable dropdown. Options: `value` |
| `RadioButton(name, value)` | Radio button. Options: `checked` |
| `Watermark(text)` | Rotated watermark. Options: `font_size`, `color`, `angle` |
| `PageBreak()` | Force a page break |
| `Fixed(*children)` | Fixed-position element. Options: `position` (`"header"`, `"footer"`) |

## Local digital signing

Apply a PKCS#7 / X.509 signature to an existing PDF, offline:

```python
import json
from formepdf.wasm import certify_pdf

with open("document.pdf", "rb") as f:
    pdf = f.read()

config = json.dumps({
    "certificate_pem": open("cert.pem").read(),
    "private_key_pem": open("key.pem").read(),
    "reason": "Approved",
})

certified = certify_pdf(pdf, config)
```

## Self-hosted render server (optional)

The package also ships a thin HTTP client, `Forme`, for a **self-hosted** Forme render
server — the public hosted API has been retired, so point `base_url` at your own
deployment. It exposes server-side operations the local WASM engine doesn't:
render stored templates by slug (`render` / `render_async` / `get_job`), true redaction
(`redact`), `merge`, `rasterize`, and `extract`.

```python
from formepdf import Forme

client = Forme("forme_sk_...", base_url="https://pdf.your-company.com")
pdf = client.render("invoice", {"customer": "Acme", "total": 245})
```

## Requirements

- Python 3.8+
- `wasmtime` for local rendering (`pip install formepdf[local]`)
- No system libraries, no browser.
