import initWasm, { render_pdf as wasmRenderPdf } from '../pkg/forme.js';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import type { ReactElement } from 'react';

// ── Layout metadata types ──────────────────────────────────────────

export interface ElementInfo {
  x: number;
  y: number;
  width: number;
  height: number;
  kind: string;
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
  const { serialize } = await import('@forme/react');
  const json = JSON.stringify(serialize(element));
  return renderPdf(json);
}

export async function renderDocumentWithLayout(element: ReactElement): Promise<RenderWithLayoutResult> {
  const { serialize } = await import('@forme/react');
  const json = JSON.stringify(serialize(element));
  return renderPdfWithLayout(json);
}
