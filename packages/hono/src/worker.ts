/**
 * Cloudflare Workers entry for @formepdf/hono, chosen by the "worker" export
 * condition (Wrangler resolves it). Workers import a .wasm file as a compiled
 * WebAssembly.Module, so this entry uses @formepdf/core's worker build and
 * instantiates it with that module. The browser build that index.ts uses on
 * the edge expects the bundler to instantiate the WASM, which Wrangler does
 * not, so c.pdf() failed on Workers with "__wbindgen_start is not a function".
 */
import { init, renderDocument } from '@formepdf/core/worker';
import wasm from '@formepdf/core/pkg-web/forme_bg.wasm';
import { setEngineLoader, type RenderDocumentFn } from './shared.js';

setEngineLoader(async () => {
  await init(wasm);
  return renderDocument as RenderDocumentFn;
});

export { pdfResponse, formePdf, listTemplates } from './shared.js';
export type { PdfOptions, FormePdfOptions, RenderDocumentFn } from './shared.js';
