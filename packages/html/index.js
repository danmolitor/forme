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
 * @param {{pageSize?: 'A4'|'A3'|'A5'|'Letter'|'Legal'|'Tabloid', pageMargin?: number, css?: string}} [options]
 *   `pageSize`/`pageMargin` override the document's own `@page` rule
 *   (print-dialog precedence); `css` is appended after the document's
 *   stylesheets, winning equal-specificity ties.
 * @returns {{pdf: Uint8Array, warnings: string[]}}
 *   `warnings` lists everything the input asked for that the documented
 *   subset doesn't cover — nothing is silently dropped.
 */
function renderHtml(html, options = {}) {
  const result = render_html_wasm(html, JSON.stringify(options));
  try {
    return { pdf: result.pdf, warnings: result.warnings };
  } finally {
    result.free();
  }
}

module.exports = { renderHtml };
