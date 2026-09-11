"""Tests for the local HTML->PDF path and its option surface.

Covers `formepdf.render_html(...)` beyond the byte-parity gate: that each
Pythonic option actually reaches the engine and shapes the output, and that
failures surface as `FormeRenderError`. Self-contained — no Node, no network.
"""

import base64
import pathlib

import pytest

from formepdf import render_html
from formepdf.wasm import FormeRenderError

REPO_ROOT = pathlib.Path(__file__).resolve().parents[3]
FONT = REPO_ROOT / "packages" / "fonts-standard" / "fonts" / "LiberationSans-Regular.ttf"

HTML = (
    "<!doctype html><html lang='en'><head><style>"
    "body{font-family:'Liberation Sans',sans-serif;font-size:12pt;margin:24pt}"
    "h1{font-size:20pt}</style></head>"
    "<body><h1>Statement</h1><p>Amounts in EUR.</p></body></html>"
)


def _pdf_ok(pdf: bytes) -> None:
    assert isinstance(pdf, (bytes, bytearray))
    assert pdf[:5] == b"%PDF-"
    assert len(pdf) > 500


def test_render_html_default_produces_pdf():
    _pdf_ok(render_html(HTML))


def test_page_size_changes_output():
    a4 = render_html(HTML, page_size="A4")
    letter = render_html(HTML, page_size="Letter")
    _pdf_ok(a4)
    _pdf_ok(letter)
    # Different media boxes => different bytes.
    assert a4 != letter


def test_unknown_page_size_raises():
    with pytest.raises(FormeRenderError):
        render_html(HTML, page_size="Postcard")


def test_css_injection_changes_output():
    base = render_html(HTML)
    withcss = render_html(HTML, css="h1{font-size:40pt}")
    _pdf_ok(withcss)
    assert base != withcss


def test_pdfa_with_embedded_font_is_pdfa():
    font = FONT.read_bytes()
    pdf = render_html(
        HTML,
        pdfa="2b",
        lang="en",
        fonts=[{"family": "Liberation Sans", "data": font, "weight": 400, "italic": False}],
    )
    _pdf_ok(pdf)
    # PDF/A stamps a pdfaid marker in the XMP metadata.
    assert b"pdfaid" in pdf, "PDF/A output is missing its pdfaid XMP marker"


def test_pdfa_without_embeddable_font_raises():
    # PDF/A requires all fonts embedded; a base-14 family that isn't supplied
    # must fail loudly rather than emit a non-conforming file.
    with pytest.raises(FormeRenderError):
        render_html(
            "<html lang='en'><body><p style=\"font-family:Helvetica\">x</p></body></html>",
            pdfa="2b",
            lang="en",
        )


def test_pdf_ua_produces_tagged_structure():
    font = FONT.read_bytes()
    pdf = render_html(
        HTML,
        pdf_ua=True,
        lang="en",
        fonts=[{"family": "Liberation Sans", "data": font, "weight": 400, "italic": False}],
    )
    _pdf_ok(pdf)
    # PDF/UA requires a tagged structure tree.
    assert b"StructTreeRoot" in pdf, "PDF/UA output has no structure tree"


def test_fonts_accept_bytes_and_base64_identically():
    raw = FONT.read_bytes()
    b64 = base64.b64encode(raw).decode("ascii")
    opts = dict(pdfa="2b", lang="en")
    from_bytes = render_html(
        HTML, fonts=[{"family": "Liberation Sans", "data": raw, "weight": 400}], **opts
    )
    from_b64 = render_html(
        HTML, fonts=[{"family": "Liberation Sans", "data": b64, "weight": 400}], **opts
    )
    # Same font, expressed two ways => identical embedded program => same bytes.
    assert from_bytes == from_b64


def test_default_is_not_pdfa():
    # Sanity: the plain render must NOT carry PDF/A conformance.
    assert b"pdfaid" not in render_html(HTML)
