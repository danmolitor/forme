/**
 * Cloudflare Workers / edge entry point for @formepdf/core.
 *
 * Backed by the wasm-pack `--target web` build of `pkg-web/`. Unlike
 * the bundler-target browser entry, this one does **not** auto-init
 * the WASM at module load — that's incompatible with Wrangler's
 * WASM-as-ESM contract (Wrangler returns `{ default: WebAssembly.Module }`
 * rather than an instantiated namespace, so the bundler glue's
 * top-level `wasm.__wbindgen_start()` would throw).
 *
 * Instead, callers pass the `WebAssembly.Module` they imported
 * themselves into `init(module)` once at request time:
 *
 *   import { init, renderDocument } from '@formepdf/core'           // resolves here under the 'worker' condition
 *   import wasm from '@formepdf/core/pkg-web/forme_bg.wasm'         // recommended
 *   // or:
 *   import wasm from '@formepdf/core/pkg/forme_bg.wasm'             // legacy, also works
 *
 *   export default {
 *     async fetch() {
 *       await init(wasm);
 *       const pdf = await renderDocument(<Doc />);
 *       return new Response(pdf, { headers: { 'content-type': 'application/pdf' } });
 *     },
 *   };
 */

import __wbg_init, {
  certify_pdf as wasmCertifyPdf,
  find_text_regions as wasmFindTextRegions,
  merge_pdfs as wasmMergePdfs,
  redact_pdf as wasmRedactPdf,
  redact_text as wasmRedactText,
  render_pdf as wasmRenderPdf,
  render_pdf_with_layout as wasmRenderPdfWithLayout,
  render_template_pdf as wasmRenderTemplatePdf,
  render_template_pdf_with_layout as wasmRenderTemplatePdfWithLayout,
} from '../pkg-web/forme.js';
import {
  resolveFonts,
  resolveImages,
  extractDataFromPdf,
} from './shared/browserHelpers.js';
import { applyAttachmentOptions, type AttachmentOptions, type FacturXOptions } from './attachments.js';

// ── Re-export types from the main entry ────────────────────────────

export type {
  Color,
  EdgeValues,
  CornerValues,
  ElementStyleInfo,
  ElementInfo,
  PageInfo,
  LayoutInfo,
  RenderWithLayoutResult,
  RenderDocumentOptions,
  RedactionRegion,
  RedactionPattern,
} from './index.js';

import type {
  LayoutInfo,
  RenderWithLayoutResult,
  RenderDocumentOptions,
  RedactionRegion,
  RedactionPattern,
} from './index.js';

// ── WASM initialization ────────────────────────────────────────────

let initPromise: Promise<unknown> | null = null;

/**
 * Initialize the WASM engine. Pass a `WebAssembly.Module` (the default
 * shape Wrangler/esbuild give you when you `import wasm from '…/forme_bg.wasm'`)
 * or any other value `__wbg_init` accepts: a `URL`, a `Response`, a
 * `Promise<Response>`, raw bytes (`BufferSource`), or `undefined` to
 * fall back to fetching `forme_bg.wasm` next to `pkg-web/forme.js`.
 *
 * Idempotent: subsequent calls reuse the first invocation's promise.
 * Must complete before any render/redact/merge call.
 */
export async function init(module?: unknown): Promise<void> {
  if (!initPromise) {
    initPromise = __wbg_init(
      module === undefined ? undefined : { module_or_path: module as never },
    );
  }
  await initPromise;
}

function ensureInit(): void {
  if (!initPromise) {
    throw new Error(
      '[@formepdf/core/worker] WASM not initialized. Call `await init(wasmModule)` ' +
        'with the `WebAssembly.Module` imported from `@formepdf/core/pkg-web/forme_bg.wasm` ' +
        'before invoking any render/redact/merge function.',
    );
  }
}

// ── Render functions ───────────────────────────────────────────────

export async function renderPdf(json: string): Promise<Uint8Array> {
  ensureInit();
  await initPromise;
  return wasmRenderPdf(json);
}

export async function renderPdfWithLayout(json: string): Promise<RenderWithLayoutResult> {
  ensureInit();
  await initPromise;
  const result = wasmRenderPdfWithLayout(json) as { pdf: Uint8Array; layout: LayoutInfo; warnings?: string[] };
  return { ...result, warnings: result.warnings ?? [] };
}

export async function renderDocument(
  element: import('react').ReactElement,
  options?: RenderDocumentOptions,
): Promise<Uint8Array> {
  const { serialize } = await import('@formepdf/react');
  const doc = serialize(element) as unknown as Record<string, unknown>;
  if (options?.embedData !== undefined) {
    doc.embeddedData = JSON.stringify(options.embedData);
  }
  if (options?.flattenForms) {
    doc.flattenForms = true;
  }
  applyAttachmentOptions(doc, options);
  await Promise.all([resolveFonts(doc), resolveImages(doc)]);
  return renderPdf(JSON.stringify(doc));
}

export async function renderDocumentWithLayout(
  element: import('react').ReactElement,
  options?: RenderDocumentOptions,
): Promise<RenderWithLayoutResult> {
  const { serialize } = await import('@formepdf/react');
  const doc = serialize(element) as unknown as Record<string, unknown>;
  if (options?.embedData !== undefined) {
    doc.embeddedData = JSON.stringify(options.embedData);
  }
  if (options?.flattenForms) {
    doc.flattenForms = true;
  }
  applyAttachmentOptions(doc, options);
  await Promise.all([resolveFonts(doc), resolveImages(doc)]);
  return renderPdfWithLayout(JSON.stringify(doc));
}

// ── Serialized document rendering ────────────────────────────────────

export async function renderSerializedDoc(
  doc: Record<string, unknown>,
  options?: RenderDocumentOptions,
): Promise<Uint8Array> {
  if (options?.embedData !== undefined) {
    doc.embeddedData = JSON.stringify(options.embedData);
  }
  if (options?.flattenForms) {
    doc.flattenForms = true;
  }
  applyAttachmentOptions(doc, options);
  await Promise.all([resolveFonts(doc), resolveImages(doc)]);
  return renderPdf(JSON.stringify(doc));
}

export async function renderSerializedDocWithLayout(
  doc: Record<string, unknown>,
  options?: RenderDocumentOptions,
): Promise<RenderWithLayoutResult> {
  if (options?.embedData !== undefined) {
    doc.embeddedData = JSON.stringify(options.embedData);
  }
  if (options?.flattenForms) {
    doc.flattenForms = true;
  }
  applyAttachmentOptions(doc, options);
  await Promise.all([resolveFonts(doc), resolveImages(doc)]);
  return renderPdfWithLayout(JSON.stringify(doc));
}

// ── Template rendering ──────────────────────────────────────────────

export async function renderTemplate(templateJson: string, dataJson: string): Promise<Uint8Array> {
  ensureInit();
  await initPromise;
  return wasmRenderTemplatePdf(templateJson, dataJson);
}

export async function renderTemplateWithLayout(
  templateJson: string,
  dataJson: string,
): Promise<RenderWithLayoutResult> {
  ensureInit();
  await initPromise;
  const result = wasmRenderTemplatePdfWithLayout(templateJson, dataJson) as {
    pdf: Uint8Array;
    layout: LayoutInfo;
    warnings?: string[];
  };
  return { ...result, warnings: result.warnings ?? [] };
}

// ── PDF certification ────────────────────────────────────────────────

export async function certifyPdf(
  pdfBytes: Uint8Array,
  config: {
    certificatePem: string;
    privateKeyPem: string;
    reason?: string;
    location?: string;
    contact?: string;
    visible?: boolean;
    page?: number;
    x?: number;
    y?: number;
    width?: number;
    height?: number;
  },
): Promise<Uint8Array> {
  ensureInit();
  await initPromise;
  return wasmCertifyPdf(pdfBytes, JSON.stringify(config));
}

/** @deprecated Use certifyPdf */
export const signPdf = certifyPdf;

// ── PDF redaction ────────────────────────────────────────────────────

export async function redactPdf(
  pdfBytes: Uint8Array,
  regions: RedactionRegion[],
): Promise<Uint8Array> {
  ensureInit();
  await initPromise;
  return wasmRedactPdf(pdfBytes, JSON.stringify(regions));
}

// ── Text-search redaction ─────────────────────────────────────────────

export async function findTextRegions(
  pdfBytes: Uint8Array,
  patterns: RedactionPattern[],
): Promise<RedactionRegion[]> {
  ensureInit();
  await initPromise;
  const json = wasmFindTextRegions(pdfBytes, JSON.stringify(patterns));
  return JSON.parse(json) as RedactionRegion[];
}

export async function redactText(
  pdfBytes: Uint8Array,
  patterns: RedactionPattern[],
): Promise<Uint8Array> {
  ensureInit();
  await initPromise;
  return wasmRedactText(pdfBytes, JSON.stringify(patterns));
}

// ── PDF merging ──────────────────────────────────────────────────────

export async function mergePdfs(pdfs: Uint8Array[]): Promise<Uint8Array> {
  ensureInit();
  await initPromise;
  const base64Pdfs = pdfs.map((pdf) =>
    btoa(Array.from(pdf, (b) => String.fromCharCode(b)).join('')),
  );
  return wasmMergePdfs(JSON.stringify(base64Pdfs));
}

// ── Data extraction ─────────────────────────────────────────────────

/** Extract embedded JSON data from a Forme-generated PDF. */
export async function extractData(pdfBytes: Uint8Array): Promise<unknown | null> {
  return extractDataFromPdf(pdfBytes);
}

export type { AttachmentOptions, FacturXOptions, AfRelationship, FacturXProfile } from './attachments.js';
