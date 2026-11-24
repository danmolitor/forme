import { setEngineLoader, type RenderDocumentFn } from './shared.js';

// Node.js uses @formepdf/core; other runtimes reached through this entry use
// @formepdf/core/browser. Cloudflare Workers resolve ./worker.ts instead, via
// the "worker" export condition.
setEngineLoader(async (): Promise<RenderDocumentFn> => {
  // Workers with nodejs_compat expose process.versions.node, but their
  // import.meta.url isn't a file:// URL — the Node entry's fileURLToPath()
  // would crash. Branch on the actual capability we need.
  const isEdge = typeof import.meta.url !== 'string' || !import.meta.url.startsWith('file://');
  if (isEdge) {
    // The browser entry is backed by wasm-pack --target bundler, so a
    // bundler that instantiates .wasm imports (Vite, webpack) wires it in
    // at module load. Wrangler does not; Workers use ./worker.ts.
    const browser = await import('@formepdf/core/browser');
    return browser.renderDocument as RenderDocumentFn;
  } else {
    const core = await import('@formepdf/core');
    return core.renderDocument as RenderDocumentFn;
  }
});

export { pdfResponse, formePdf, listTemplates } from './shared.js';
export type { PdfOptions, FormePdfOptions, RenderDocumentFn } from './shared.js';
