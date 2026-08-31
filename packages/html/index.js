// @formepdf/html — HTML + print-CSS to PDF, no headless browser.
//
// Node / npx entry, backed by the wasm-pack `--target nodejs` build in
// pkg-node/. That build is a self-initializing CommonJS module (it
// `require('fs').readFileSync`s its own .wasm at load), so there is no init
// step — the named imports below are live the moment this module evaluates.
//
// For Cloudflare Workers / edge import `@formepdf/html/worker`; for a browser
// bundler (Vite, Webpack, esbuild) import `@formepdf/html/browser`. All three
// entries expose the identical render API and route to the same WASM engine.

import {
  render_html_wasm,
  render_html_wasm_with_layout,
} from './pkg-node/forme_pdf_html.js';
import { toWireOptions } from './wire.js';

/**
 * No-op on Node: the nodejs target self-initializes. Present so the three
 * entries share one surface; only the worker entry needs a real `init`.
 * @returns {Promise<void>}
 */
export async function init() {}

/**
 * Render an HTML string to PDF.
 *
 * @param {string} html
 * @param {import('./index').RenderHtmlOptions} [options]
 * @returns {{pdf: Uint8Array, warnings: string[]}}
 *   `warnings` lists everything the input asked for that the documented
 *   subset doesn't cover — nothing is silently dropped.
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
 * Render an HTML string to PDF *plus* its `LayoutInfo` — the laid-out node
 * tree that drives tooling (the VS Code extension's component tree,
 * inspector, and layout overlays). The `layout` is the identical
 * `LayoutInfo` shape the core engine emits for JSX, so a consumer can't tell
 * HTML-sourced output from JSX-sourced output.
 *
 * @param {string} html
 * @param {import('./index').RenderHtmlOptions} [options]
 * @returns {{pdf: Uint8Array, layout: import('./index').LayoutInfo, warnings: string[]}}
 */
export function renderHtmlWithLayout(html, options = {}) {
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
