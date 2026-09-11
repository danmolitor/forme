"""FAILS-FIRST — the claim under test: the Python HTML path IS the real engine.

`formepdf.render_html(html, **opts)` must produce the BYTE-IDENTICAL PDF that
Node's `@formepdf/html` `renderHtml(html, opts)` produces for the same input.
Both are the same Rust engine (the `html` crate); the Python side runs it as a
wasm32-wasip1 module via wasmtime, Node runs it via wasm-bindgen. The engine is
a deterministic pure function of its input (the determinism gate already proves
node == web == native byte-for-byte), so wasip1 must match too.

This is red until `forme_render_html` is exported over the C-ABI and bound in
`wasm.py`. If it ever renders something that ISN'T the engine's real output,
this goes red — which is exactly the point.
"""

import json
import os
import pathlib
import subprocess

import pytest

# Repo root: tests/ -> python-sdk -> packages -> <repo root>
REPO_ROOT = pathlib.Path(__file__).resolve().parents[3]
# Where to resolve Node's @formepdf/html for the reference render. Defaults to
# the repo root (CI builds @formepdf/html there); override for a local checkout
# whose node_modules lives elsewhere.
NODE_REF_DIR = os.environ.get("FORME_NODE_REF_DIR", str(REPO_ROOT))

# A real embeddable TTF both sides can share by path (PDF/A requires embedded
# fonts), so the pdfa parity case renders identical bytes on each side.
FONT = REPO_ROOT / "packages" / "fonts-standard" / "fonts" / "LiberationSans-Regular.ttf"

HTML = """<!doctype html><html lang="en"><head><style>
  body { font-family: 'Helvetica', sans-serif; font-size: 12pt; margin: 24pt; }
  h1 { font-size: 20pt; }
  table { width: 100%; border-collapse: collapse; margin-top: 12pt; }
  td, th { border: 1px solid #333; padding: 4pt; text-align: left; }
</style></head><body>
  <h1>Invoice INV-001</h1>
  <p>Prepared for Mandare. Amounts in EUR.</p>
  <table>
    <thead><tr><th>Item</th><th>Qty</th><th>Amount</th></tr></thead>
    <tbody>
      <tr><td>Rendering seat</td><td>3</td><td>&#8364;30.00</td></tr>
      <tr><td>Support</td><td>1</td><td>&#8364;99.00</td></tr>
    </tbody>
  </table>
</body></html>"""


def _node_render_html(html: str) -> bytes:
    """Reference render via Node @formepdf/html, resolved from the repo root."""
    script = (
        "let h='';process.stdin.on('data',d=>h+=d).on('end',async()=>{"
        "const {renderHtml}=await import('@formepdf/html');"
        "const {pdf}=renderHtml(h,{});"
        "process.stdout.write(Buffer.from(pdf));});"
    )
    r = subprocess.run(
        ["node", "--input-type=module", "-e", script],
        input=html.encode("utf-8"),
        capture_output=True,
        cwd=NODE_REF_DIR,
    )
    if r.returncode != 0:
        pytest.skip(f"Node @formepdf/html reference unavailable: {r.stderr.decode()[:200]}")
    return r.stdout


def test_python_render_html_is_byte_identical_to_node():
    # RED until implemented: `render_html` is not exported by formepdf yet.
    from formepdf import render_html

    py_pdf = render_html(HTML)
    node_pdf = _node_render_html(HTML)

    assert isinstance(py_pdf, (bytes, bytearray))
    assert py_pdf[:5] == b"%PDF-", "python output is not a PDF"
    assert bytes(py_pdf) == node_pdf, (
        f"python HTML render is not byte-identical to Node: "
        f"{len(py_pdf)} vs {len(node_pdf)} bytes"
    )


# For the PDF/A case: reference the embedded family directly (PDF/A can't use a
# non-embedded base-14 name like Helvetica), so the registered font actually
# satisfies conformance on both sides.
HTML_PDFA = """<!doctype html><html lang="en"><head><style>
  body { font-family: 'Liberation Sans', sans-serif; font-size: 12pt; margin: 24pt; }
  h1 { font-size: 20pt; }
</style></head><body>
  <h1>Accessible Invoice</h1>
  <p>Archival copy. Amounts in EUR.</p>
</body></html>"""


def _node_render_html_pdfa(html: str, font_path: pathlib.Path) -> bytes:
    """Reference render with PDF/A-2b + an embedded font, via Node."""
    script = (
        "import {readFileSync} from 'node:fs';"
        "let h='';process.stdin.on('data',d=>h+=d).on('end',async()=>{"
        "const {renderHtml}=await import('@formepdf/html');"
        f"const font=readFileSync({json.dumps(str(font_path))});"
        "const {pdf}=renderHtml(h,{pdfA:'2b',lang:'en',"
        "fonts:[{family:'Liberation Sans',data:font,weight:400,italic:false}]});"
        "process.stdout.write(Buffer.from(pdf));});"
    )
    r = subprocess.run(
        ["node", "--input-type=module", "-e", script],
        input=html.encode("utf-8"),
        capture_output=True,
        cwd=NODE_REF_DIR,
    )
    if r.returncode != 0:
        pytest.skip(f"Node @formepdf/html reference unavailable: {r.stderr.decode()[:200]}")
    return r.stdout


def test_python_render_html_with_pdfa_and_fonts_is_byte_identical_to_node():
    # RED until options are threaded: render_html takes no kwargs yet.
    from formepdf import render_html

    font = FONT.read_bytes()
    py_pdf = render_html(
        HTML_PDFA,
        pdfa="2b",
        lang="en",
        fonts=[{"family": "Liberation Sans", "data": font, "weight": 400, "italic": False}],
    )
    node_pdf = _node_render_html_pdfa(HTML_PDFA, FONT)

    assert py_pdf[:5] == b"%PDF-", "python output is not a PDF"
    assert bytes(py_pdf) == node_pdf, (
        f"python PDF/A+fonts render is not byte-identical to Node: "
        f"{len(py_pdf)} vs {len(node_pdf)} bytes"
    )
