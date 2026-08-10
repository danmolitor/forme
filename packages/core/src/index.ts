// Node entry uses the wasm-pack `--target nodejs` build, which is a
// self-initializing CJS module: it `require('fs').readFileSync`s its
// own .wasm at import time, so callers don't need to await any init.
import { render_pdf as wasmRenderPdf } from '../pkg-node/forme.js';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import type { ReactElement } from 'react';

// ── Layout metadata types ──────────────────────────────────────────

export interface Color {
  r: number;
  g: number;
  b: number;
  a: number;
}

export interface EdgeValues<T> {
  top: T;
  right: T;
  bottom: T;
  left: T;
}

export interface CornerValues {
  top_left: number;
  top_right: number;
  bottom_right: number;
  bottom_left: number;
}

export interface ElementStyleInfo {
  margin: EdgeValues<number>;
  padding: EdgeValues<number>;
  borderWidth: EdgeValues<number>;
  flexDirection: string;
  justifyContent: string;
  alignItems: string;
  flexWrap: string;
  gap: number;
  fontFamily: string;
  fontSize: number;
  fontWeight: number;
  fontStyle: string;
  lineHeight: number;
  textAlign: string;
  color: Color;
  backgroundColor: Color | null;
  borderColor: EdgeValues<Color>;
  borderRadius: CornerValues;
  opacity: number;
}

export interface ElementInfo {
  x: number;
  y: number;
  width: number;
  height: number;
  kind: string;
  nodeType: string;
  style: ElementStyleInfo;
  children: ElementInfo[];
  sourceLocation?: { file: string; line: number; column: number };
  textContent?: string;
}

export interface PageInfo {
  width: number;
  height: number;
  contentX: number;
  contentY: number;
  contentWidth: number;
  contentHeight: number;
  elements: ElementInfo[];
}

export interface LayoutInfo {
  pages: PageInfo[];
}

export interface RenderWithLayoutResult {
  pdf: Uint8Array;
  layout: LayoutInfo;
}

// ── Font resolution ──────────────────────────────────────────────

function uint8ArrayToBase64(bytes: Uint8Array): string {
  return Buffer.from(bytes).toString('base64');
}

async function resolveFonts(doc: Record<string, unknown>): Promise<void> {
  const fonts = doc.fonts as Array<{ family: string; src: string | Uint8Array; weight: number; italic: boolean }> | undefined;
  if (!fonts?.length) return;

  for (const font of fonts) {
    if (font.src instanceof Uint8Array) {
      font.src = uint8ArrayToBase64(font.src);
    } else if (typeof font.src === 'string' && !font.src.startsWith('data:')) {
      const bytes = await readFile(resolve(font.src));
      font.src = uint8ArrayToBase64(new Uint8Array(bytes));
    }
    // data URIs pass through as-is (engine extracts base64 portion)
  }
}

// ── Image resolution ─────────────────────────────────────────────

async function resolveImages(doc: Record<string, unknown>): Promise<void> {
  const children = doc.children as Array<Record<string, unknown>> | undefined;
  if (!children?.length) return;
  for (const child of children) {
    await resolveImagesInNode(child);
  }
}

async function resolveImagesInNode(node: Record<string, unknown>): Promise<void> {
  const kind = node.kind as Record<string, unknown> | undefined;
  if (kind?.type === 'Image' && typeof kind.src === 'string') {
    const src = kind.src as string;
    if (src.startsWith('http://') || src.startsWith('https://')) {
      const res = await fetch(src);
      if (!res.ok) throw new Error(`Failed to fetch image: ${src} (${res.status})`);
      const contentType = res.headers.get('content-type') || 'image/png';
      const buf = new Uint8Array(await res.arrayBuffer());
      kind.src = `data:${contentType};base64,${uint8ArrayToBase64(buf)}`;
    }
  }
  const children = node.children as Array<Record<string, unknown>> | undefined;
  if (children?.length) {
    for (const child of children) {
      await resolveImagesInNode(child);
    }
  }
}

// ── Render functions ───────────────────────────────────────────────

export async function renderPdf(json: string): Promise<Uint8Array> {
  return wasmRenderPdf(json);
}

export async function renderPdfWithLayout(json: string): Promise<RenderWithLayoutResult> {
  const { render_pdf_with_layout } = await import('../pkg-node/forme.js');
  const result = render_pdf_with_layout(json) as { pdf: Uint8Array; layout: LayoutInfo };
  return result;
}

export interface CertificationConfig {
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
}

/** @deprecated Use CertificationConfig */
export type SignatureConfig = CertificationConfig;

export interface RenderDocumentOptions {
  /** Data to embed as a hidden JSON attachment in the PDF. */
  embedData?: unknown;
  /** When true, form field values are rendered as static text. No interactive fields in output. */
  flattenForms?: boolean;
}

export async function renderDocument(element: ReactElement, options?: RenderDocumentOptions): Promise<Uint8Array> {
  const { serialize } = await import('@formepdf/react');
  const doc = serialize(element) as unknown as Record<string, unknown>;
  return renderSerializedDoc(doc, options);
}

export async function renderDocumentWithLayout(element: ReactElement, options?: RenderDocumentOptions): Promise<RenderWithLayoutResult> {
  const { serialize } = await import('@formepdf/react');
  const doc = serialize(element) as unknown as Record<string, unknown>;
  return renderSerializedDocWithLayout(doc, options);
}

// ── Serialized document rendering ────────────────────────────────────

/**
 * Render a pre-serialized document object (from `serialize()`) to PDF,
 * resolving font sources (file paths, byte arrays) and HTTP image URLs
 * first.
 *
 * Use this when you have a serialized doc (e.g. from a non-react
 * adapter that calls its own `serialize()`) and need font/image
 * resolution without going through the React element-based
 * `renderDocument()`. The browser and worker entries export the same
 * pair.
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
  await Promise.all([resolveFonts(doc), resolveImages(doc)]);
  return renderPdfWithLayout(JSON.stringify(doc));
}

// ── Template rendering ──────────────────────────────────────────────

export async function renderTemplate(templateJson: string, dataJson: string): Promise<Uint8Array> {
  const { render_template_pdf } = await import('../pkg-node/forme.js');
  return render_template_pdf(templateJson, dataJson);
}

export async function renderTemplateWithLayout(templateJson: string, dataJson: string): Promise<RenderWithLayoutResult> {
  const { render_template_pdf_with_layout } = await import('../pkg-node/forme.js');
  const result = render_template_pdf_with_layout(templateJson, dataJson) as { pdf: Uint8Array; layout: LayoutInfo };
  return result;
}

// ── PDF certification ────────────────────────────────────────────────

export async function certifyPdf(pdfBytes: Uint8Array, config: CertificationConfig): Promise<Uint8Array> {
  const { certify_pdf } = await import('../pkg-node/forme.js');
  return certify_pdf(pdfBytes, JSON.stringify(config));
}

/** @deprecated Use certifyPdf */
export const signPdf = certifyPdf;

// ── PDF redaction ────────────────────────────────────────────────────

export interface RedactionRegion {
  /** 0-indexed page number. */
  page: number;
  /** X coordinate in points from the left edge. */
  x: number;
  /** Y coordinate in points from the top edge (web/screen coordinates). */
  y: number;
  /** Width of the redaction rectangle in points. */
  width: number;
  /** Height of the redaction rectangle in points. */
  height: number;
  /** Fill color as hex string (e.g. "#000000"). Defaults to black. */
  color?: string;
}

export async function redactPdf(pdfBytes: Uint8Array, regions: RedactionRegion[]): Promise<Uint8Array> {
  const { redact_pdf } = await import('../pkg-node/forme.js');
  return redact_pdf(pdfBytes, JSON.stringify(regions));
}

// ── Text-search redaction ─────────────────────────────────────────────

export interface RedactionPattern {
  /** The text or regex pattern to search for. */
  pattern: string;
  /** 'Literal' for exact text match (case-insensitive), 'Regex' for regex. */
  pattern_type: 'Literal' | 'Regex';
  /** Optional 0-indexed page to restrict search to. */
  page?: number;
  /** Fill color as hex string (e.g. "#000000"). Defaults to black. */
  color?: string;
}

/**
 * Find text regions matching patterns in a PDF.
 *
 * Searches PDF content streams for literal or regex patterns and returns
 * redaction regions (in web top-origin coordinates) for each match.
 */
export async function findTextRegions(pdfBytes: Uint8Array, patterns: RedactionPattern[]): Promise<RedactionRegion[]> {
  const { find_text_regions } = await import('../pkg-node/forme.js');
  const json = find_text_regions(pdfBytes, JSON.stringify(patterns));
  return JSON.parse(json) as RedactionRegion[];
}

/**
 * Redact text matching patterns from a PDF.
 *
 * Convenience wrapper: finds all text matching the patterns, then
 * applies coordinate-based redaction to each match.
 */
export async function redactText(pdfBytes: Uint8Array, patterns: RedactionPattern[]): Promise<Uint8Array> {
  const { redact_text } = await import('../pkg-node/forme.js');
  return redact_text(pdfBytes, JSON.stringify(patterns));
}

// ── PDF merging ──────────────────────────────────────────────────────

/**
 * Merge multiple PDF documents into a single PDF.
 *
 * @param pdfs - Array of PDF byte arrays to merge in order.
 * @returns The merged PDF as a Uint8Array.
 */
export async function mergePdfs(pdfs: Uint8Array[]): Promise<Uint8Array> {
  const { merge_pdfs } = await import('../pkg-node/forme.js');
  const base64Pdfs = pdfs.map((pdf) => Buffer.from(pdf).toString('base64'));
  return merge_pdfs(JSON.stringify(base64Pdfs));
}

// ── Data extraction ──────────────────────────────────────────────────

export { extractData } from './extract.js';
