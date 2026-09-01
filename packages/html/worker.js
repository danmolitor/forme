// @formepdf/html — Cloudflare Workers / edge entry.
//
// Import as `@formepdf/html/worker`. Backed by the wasm-pack `--target web`
// build in pkg-web/. Unlike the bundler entry, this one does NOT auto-init
// the WASM at module load — that's incompatible with Wrangler's WASM-as-ESM
// contract (it hands you a `WebAssembly.Module`, not an instantiated
// namespace). Call `init(module)` once at request time, then render:
//
//   import { init, renderHtml } from '@formepdf/html/worker'
//   import wasm from '@formepdf/html/pkg-web/forme_pdf_html_bg.wasm'
//
//   export default {
//     async fetch() {
//       await init(wasm);
//       const { pdf } = renderHtml('<h1>Hi</h1>');
//       return new Response(pdf, { headers: { 'content-type': 'application/pdf' } });
//     },
//   };

import __wbg_init, {
  render_html_wasm,
  render_html_wasm_with_layout,
} from './pkg-web/forme_pdf_html.js';
import { toWireOptions } from './wire.js';

let initPromise = null;

/**
 * Initialize the WASM engine. Pass the `WebAssembly.Module` you imported
 * (the default shape Wrangler/esbuild give you for a `.wasm` import), or any
 * value `__wbg_init` accepts: a `URL`, `Response`, `Promise<Response>`, raw
 * bytes, or `undefined` to fetch `forme_pdf_html_bg.wasm` next to the module.
 *
 * Idempotent — later calls reuse the first invocation's promise. Must resolve
 * before any `renderHtml` / `renderHtmlWithLayout` call.
 * @param {unknown} [module]
 * @returns {Promise<void>}
 */
export async function init(module) {
  if (!initPromise) {
    initPromise = __wbg_init(module === undefined ? undefined : { module_or_path: module });
  }
  await initPromise;
}

function ensureInit() {
  if (!initPromise) {
    throw new Error(
      '[@formepdf/html/worker] WASM not initialized. Call `await init(wasmModule)` ' +
        'with the `WebAssembly.Module` imported from ' +
        '`@formepdf/html/pkg-web/forme_pdf_html_bg.wasm` before rendering.',
    );
  }
}

/**
 * Render an HTML string to PDF. Requires a prior `await init(module)`.
 * @param {string} html
 * @param {import('./index').RenderHtmlOptions} [options]
 * @returns {{pdf: Uint8Array, warnings: string[]}}
 */
export function renderHtml(html, options = {}) {
  ensureInit();
  const result = render_html_wasm(html, JSON.stringify(toWireOptions(options)));
  try {
    return { pdf: result.pdf, warnings: result.warnings };
  } finally {
    result.free();
  }
}

/**
 * Render an HTML string to PDF plus its `LayoutInfo`. Requires a prior
 * `await init(module)`.
 * @param {string} html
 * @param {import('./index').RenderHtmlOptions} [options]
 * @returns {{pdf: Uint8Array, layout: import('./index').LayoutInfo, warnings: string[]}}
 */
export function renderHtmlWithLayout(html, options = {}) {
  ensureInit();
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
