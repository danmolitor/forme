import initWasm, { render_pdf as wasmRenderPdf } from '../pkg/forme.js';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import type { ReactElement } from 'react';

let initialized = false;

async function ensureInit(): Promise<void> {
  if (initialized) return;
  const __dirname = dirname(fileURLToPath(import.meta.url));
  const wasmPath = join(__dirname, '..', 'pkg', 'forme_bg.wasm');
  const wasmBytes = await readFile(wasmPath);
  await initWasm({ module_or_path: wasmBytes });
  initialized = true;
}

export async function renderPdf(json: string): Promise<Uint8Array> {
  await ensureInit();
  return wasmRenderPdf(json);
}

export async function renderDocument(element: ReactElement): Promise<Uint8Array> {
  const { serialize } = await import('@forme/react');
  const json = JSON.stringify(serialize(element));
  return renderPdf(json);
}
