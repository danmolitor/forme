// @formepdf/html — browser / bundler entry.
//
// Import as `@formepdf/html/browser`. Backed by the wasm-pack
// `--target bundler` build in pkg/, so the consuming bundler (Vite, Webpack,
// Turbopack, esbuild) wires up and instantiates the WASM implicitly at
// module-load time — there is no explicit init step. No Node APIs are used,
// so this runs in any browser or edge runtime with WebAssembly support.

import {
  render_html_wasm,
  render_html_wasm_with_layout,
} from './pkg/forme_pdf_html.js';
import { toWireOptions } from './wire.js';

/**
 * No-op under the bundler-target build: the WASM is already instantiated by
 * the time any export below can be called. Present for surface parity with
 * the worker entry, which needs a real `init(module)`.
 * @returns {Promise<void>}
 */
export async function init() {}

/**
 * Render an HTML string to PDF.
 * @param {string} html
 * @param {import('./index').RenderHtmlOptions} [options]
 * @returns {{pdf: Uint8Array, warnings: string[]}}
 */
export function renderHtml(html, options = {}) {
  const result = render_html_wasm(html, JSON.stringify(toWireOptions(options)));
  try {
    return { pdf: result.pdf, warnings: result.warnings };
  } finally {
    result.free();
  }
}

/**
 * Render an HTML string to PDF plus its `LayoutInfo`.
 * @param {string} html
 * @param {import('./index').RenderHtmlOptions} [options]
 * @returns {{pdf: Uint8Array, layout: import('./index').LayoutInfo, warnings: string[]}}
 */
export function renderHtmlWithLayout(html, options = {}) {
  const result = render_html_wasm_with_layout(html, JSON.stringify(toWireOptions(options)));
  try {
    return {
      pdf: result.pdf,
      layout: JSON.parse(result.layout_json),
      warnings: result.warnings,
    };
  } finally {
    result.free();
  }
}
