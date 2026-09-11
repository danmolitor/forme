# Changelog — formepdf (Python SDK)

## [0.23.0] - 2026-09-11

### Added

- **Local HTML → PDF, no system libraries.** `render_html(html, ...)` renders
  HTML/CSS to a PDF in-process through the engine compiled to `wasm32-wasip1`
  and run via `wasmtime` — no cairo, no pango, no system libraries, no browser.
  `pip install formepdf[local]` pulls a single dependency (`wasmtime`). The
  option surface: `page_size`, `page_margin`, `css`, `fonts` (bytes or base64),
  `tagged`, `pdf_ua`, `pdfa`, `lang`, `audit_content`.
- **Byte-identical to `@formepdf/html`.** The Python path is the same Rust
  engine as the JavaScript packages, not a re-implementation. A CI job renders
  the same input and options through both and requires identical bytes on every
  commit — including the PDF/A-2b-with-embedded-fonts case. It is a regression
  if that ever stops being true.
- **Python test suite** covering the `render_html` option surface (page size,
  CSS injection, embedded-font PDF/A, tagged/PDF-UA structure) and the
  byte-parity gate.

### Notes

- `redact`, `merge`, and `extract` are bound in the JavaScript packages but are
  not yet available on the local Python path.
