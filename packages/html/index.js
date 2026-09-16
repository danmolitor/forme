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
  source_hash,
} from './pkg-node/forme_pdf_html.js';
import { toWireOptions } from './wire.js';
import { toRenderResult, toLayoutResult } from './result.js';

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
 * @returns {import('./index').RenderHtmlResult}
 *   `warnings` lists everything the input asked for that the documented
 *   subset doesn't cover — nothing is silently dropped.
 */
export function renderHtml(html, options = {}) {
  return toRenderResult(render_html_wasm(html, JSON.stringify(toWireOptions(options))));
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
 * @returns {import('./index').RenderHtmlLayoutResult}
 */
export function renderHtmlWithLayout(html, options = {}) {
  return toLayoutResult(render_html_wasm_with_layout(html, JSON.stringify(toWireOptions(options))));
}

/**
 * Hash of the `engine/src` + `html/src` source this wasm was built from, or
 * an empty string if it was built without one.
 *
 * Build provenance, not part of the render API. It exists so a caller can ask
 * which engine a build came from instead of assuming: the docs gallery gate
 * hashed the source, rendered with whatever wasm happened to be built, and
 * never checked the two corresponded, which let it pass while the committed
 * images were stale. Exposed on all three entries so the surface stays
 * identical across targets.
 * @returns {string}
 */
export function sourceHash() {
  return source_hash();
}
