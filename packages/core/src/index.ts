import initWasm, { render_pdf as wasmRenderPdf } from '../pkg/forme.js';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';
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

// ── WASM initialization ────────────────────────────────────────────

let initialized = false;

async function ensureInit(): Promise<void> {
  if (initialized) return;
  const __dirname = dirname(fileURLToPath(import.meta.url));
  const wasmPath = join(__dirname, '..', 'pkg', 'forme_bg.wasm');
  const wasmBytes = await readFile(wasmPath);
  await initWasm({ module_or_path: wasmBytes });
  initialized = true;
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

// ── Render functions ───────────────────────────────────────────────

export async function renderPdf(json: string): Promise<Uint8Array> {
  await ensureInit();
  return wasmRenderPdf(json);
}

export async function renderPdfWithLayout(json: string): Promise<RenderWithLayoutResult> {
  await ensureInit();
  // Dynamic import to access the WASM binding that returns { pdf, layout }
  const { render_pdf_with_layout } = await import('../pkg/forme.js');
  const result = render_pdf_with_layout(json) as { pdf: Uint8Array; layout: LayoutInfo };
  return result;
}

export async function renderDocument(element: ReactElement): Promise<Uint8Array> {
  const { serialize } = await import('@formepdf/react');
  const doc = serialize(element) as unknown as Record<string, unknown>;
  await resolveFonts(doc);
  return renderPdf(JSON.stringify(doc));
}

export async function renderDocumentWithLayout(element: ReactElement): Promise<RenderWithLayoutResult> {
  const { serialize } = await import('@formepdf/react');
  const doc = serialize(element) as unknown as Record<string, unknown>;
  await resolveFonts(doc);
  return renderPdfWithLayout(JSON.stringify(doc));
}
