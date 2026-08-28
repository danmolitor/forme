// @formepdf/html — HTML + print-CSS to PDF, no headless browser.
//
// CommonJS on purpose: the wasm-pack nodejs target emits CJS that
// self-initializes at require time, and Node interops this cleanly from
// both require() and import.

const { render_html_wasm } = require('./pkg-node/forme_pdf_html.js');

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
  const wireOptions = { ...options };
  if (options.fonts) {
    wireOptions.fonts = options.fonts.map((f) => ({
      ...f,
      data: f.data instanceof Uint8Array ? Buffer.from(f.data).toString('base64') : f.data,
    }));
  }
  const result = render_html_wasm(html, JSON.stringify(wireOptions));
  try {
    return { pdf: result.pdf, warnings: result.warnings };
  } finally {
    result.free();
  }
}

module.exports = { renderHtml };
