/**
 * Browser / edge entry point for @formepdf/core.
 *
 * Import as `@formepdf/core/browser` — no Node APIs, works in any
 * modern browser, edge runtime, or worker with WebAssembly support.
 *
 * Backed by the wasm-pack `--target bundler` build of pkg/, so the
 * WASM module is wired up implicitly by the consuming bundler at
 * module-load time (via `import * as wasm from './forme_bg.wasm'`
 * inside pkg/forme.js). Vite, esbuild, Webpack, Turbopack, and
 * Wrangler all handle this — there is no explicit init step.
 */

import {
  certify_pdf as wasmCertifyPdf,
  find_text_regions as wasmFindTextRegions,
  merge_pdfs as wasmMergePdfs,
  redact_pdf as wasmRedactPdf,
  redact_text as wasmRedactText,
  render_pdf as wasmRenderPdf,
  render_pdf_with_layout as wasmRenderPdfWithLayout,
  render_template_pdf as wasmRenderTemplatePdf,
  render_template_pdf_with_layout as wasmRenderTemplatePdfWithLayout,
} from '../pkg/forme.js';
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

import type { LayoutInfo, RenderWithLayoutResult, RenderDocumentOptions, RedactionRegion, RedactionPattern } from './index.js';

// ── WASM initialization ────────────────────────────────────────────
//
// Kept as a no-op for backward compatibility. The bundler-target build
// instantiates the WASM at module-load time, so by the time anyone can
// invoke any of the exports below, the engine is already live.
//
// If you need an explicit `init(module)` driven by a `WebAssembly.Module`
// you imported yourself (e.g. Cloudflare Workers), import from
// `@formepdf/core/worker` instead — that entry is backed by the web
// target which supports manual instantiation.
/** @deprecated No-op under the bundler-target build. For explicit
 *  instantiation in Workers/edge, import from `@formepdf/core/worker`. */
export async function init(_module?: unknown): Promise<void> {
  return;
}

// ── Render functions ───────────────────────────────────────────────

export async function renderPdf(json: string): Promise<Uint8Array> {
  return wasmRenderPdf(json);
}

export async function renderPdfWithLayout(json: string): Promise<RenderWithLayoutResult> {
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

/**
 * Render a pre-serialized document object (from `serialize()`) to PDF,
 * resolving any HTTP image/font URLs to data URIs first.
 *
 * Use this when you have a serialized doc (e.g. from a web worker that
 * calls `serialize()` directly) and need image resolution without going
 * through the React element–based `renderDocument()`.
 */
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

/**
 * Like `renderSerializedDoc` but also returns layout info for overlays.
 */
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

export async function renderTemplate(
  templateJson: string,
  dataJson: string,
): Promise<Uint8Array> {
  return wasmRenderTemplatePdf(templateJson, dataJson);
}

export async function renderTemplateWithLayout(
  templateJson: string,
  dataJson: string,
): Promise<RenderWithLayoutResult> {
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
  config: { certificatePem: string; privateKeyPem: string; reason?: string; location?: string; contact?: string; visible?: boolean; page?: number; x?: number; y?: number; width?: number; height?: number },
): Promise<Uint8Array> {
  return wasmCertifyPdf(pdfBytes, JSON.stringify(config));
}

/** @deprecated Use certifyPdf */
export const signPdf = certifyPdf;

// ── PDF redaction ────────────────────────────────────────────────────

export async function redactPdf(
  pdfBytes: Uint8Array,
  regions: RedactionRegion[],
): Promise<Uint8Array> {
  return wasmRedactPdf(pdfBytes, JSON.stringify(regions));
}

// ── Text-search redaction ─────────────────────────────────────────────

/**
 * Find text regions matching patterns in a PDF.
 *
 * Searches PDF content streams for literal or regex patterns and returns
 * redaction regions (in web top-origin coordinates) for each match.
 */
export async function findTextRegions(
  pdfBytes: Uint8Array,
  patterns: RedactionPattern[],
): Promise<RedactionRegion[]> {
  const json = wasmFindTextRegions(pdfBytes, JSON.stringify(patterns));
  return JSON.parse(json) as RedactionRegion[];
}

/**
 * Redact text matching patterns from a PDF.
 *
 * Convenience wrapper: finds all text matching the patterns, then
 * applies coordinate-based redaction to each match.
 */
export async function redactText(
  pdfBytes: Uint8Array,
  patterns: RedactionPattern[],
): Promise<Uint8Array> {
  return wasmRedactText(pdfBytes, JSON.stringify(patterns));
}

// ── PDF merging ──────────────────────────────────────────────────────

/**
 * Merge multiple PDF documents into a single PDF.
 *
 * @param pdfs - Array of PDF byte arrays to merge in order.
 * @returns The merged PDF as a Uint8Array.
 */
export async function mergePdfs(pdfs: Uint8Array[]): Promise<Uint8Array> {
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
