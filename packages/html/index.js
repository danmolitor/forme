// @formepdf/html — HTML + print-CSS to PDF, no headless browser.
//
// CommonJS on purpose: the wasm-pack nodejs target emits CJS that
// self-initializes at require time, and Node interops this cleanly from
// both require() and import.

const {
  render_html_wasm,
  render_html_wasm_with_layout,
} = require('./pkg-node/forme_pdf_html.js');

// The engine's fonts/pageSize wire-encoding is identical for both entry
// points; keep it in one place.
function toWireOptions(options) {
  const wireOptions = { ...options };
  if (options.fonts) {
    wireOptions.fonts = options.fonts.map((f) => ({
      ...f,
      data: f.data instanceof Uint8Array ? Buffer.from(f.data).toString('base64') : f.data,
    }));
  }
  return wireOptions;
}

/**
 * Render an HTML string to PDF.
 *
 * @param {string} html
 * @param {{pageSize?: 'A4'|'A3'|'A5'|'Letter'|'Legal'|'Tabloid', pageMargin?: number, css?: string,
 *   fonts?: Array<{family: string, data: Uint8Array|string, weight?: number, italic?: boolean}>}} [options]
 *   `pageSize`/`pageMargin` override the document's own `@page` rule
 *   (print-dialog precedence); `css` is appended after the document's
 *   stylesheets, winning equal-specificity ties; `fonts` registers TTF
 *   bytes (Uint8Array or base64 string) under the family name templates
 *   reference — the offline web-font recipe.
 * @returns {{pdf: Uint8Array, warnings: string[]}}
 *   `warnings` lists everything the input asked for that the documented
 *   subset doesn't cover — nothing is silently dropped.
 */
function renderHtml(html, options = {}) {
  const result = render_html_wasm(html, JSON.stringify(toWireOptions(options)));
  try {
    return { pdf: result.pdf, warnings: result.warnings };
  } finally {
    result.free();
  }
}

/**
 * Render an HTML string to PDF *plus* its `LayoutInfo` — the laid-out node
 * tree that drives tooling (the VS Code extension's component tree,
 * inspector, and layout overlays). The `layout` is the identical
 * `LayoutInfo` shape the core engine emits for JSX, so a consumer can't
 * tell HTML-sourced output from JSX-sourced output.
 *
 * @param {string} html
 * @param {import('./index').RenderHtmlOptions} [options]
 * @returns {{pdf: Uint8Array, layout: import('@formepdf/core').LayoutInfo, warnings: string[]}}
 */
function renderHtmlWithLayout(html, options = {}) {
  const result = render_html_wasm_with_layout(html, JSON.stringify(toWireOptions(options)));
  try {
    return {
      pdf: result.pdf,
      // Returned as a JSON string from WASM (the crate's wasm feature omits
      // serde-wasm-bindgen on purpose); parse it back to the native object.
      layout: JSON.parse(result.layout_json),
      warnings: result.warnings,
    };
  } finally {
    result.free();
  }
}

module.exports = { renderHtml, renderHtmlWithLayout };
