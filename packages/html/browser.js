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
import { toRenderResult, toLayoutResult } from './result.js';

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
 * @returns {import('./index').RenderHtmlResult}
 */
export function renderHtml(html, options = {}) {
  return toRenderResult(render_html_wasm(html, JSON.stringify(toWireOptions(options))));
}

/**
 * Render an HTML string to PDF plus its `LayoutInfo`.
 * @param {string} html
 * @param {import('./index').RenderHtmlOptions} [options]
 * @returns {import('./index').RenderHtmlLayoutResult}
 */
export function renderHtmlWithLayout(html, options = {}) {
  return toLayoutResult(render_html_wasm_with_layout(html, JSON.stringify(toWireOptions(options))));
}
