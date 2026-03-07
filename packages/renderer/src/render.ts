import { writeFile, unlink } from 'node:fs/promises';
import { resolve, dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import type { ReactElement } from 'react';
import { renderPdfWithLayout, type LayoutInfo } from '@formepdf/core';
import { serialize as defaultSerialize } from '@formepdf/react';
import { bundleFile } from './bundle.js';
import { resolveElement, type ResolveElementOptions } from './element.js';
import { resolveAllSources } from './resolve.js';

export interface RenderOptions {
  dataPath?: string;
  data?: unknown;
  pageSize?: { width: number; height: number };
}

export interface RenderResult {
  pdf: Uint8Array;
  layout: LayoutInfo;
  renderTimeMs: number;
}

/// Full pipeline: bundle TSX file → resolve element → serialize → resolve assets → WASM render.
export async function renderFromFile(
  filePath: string,
  options?: RenderOptions,
): Promise<RenderResult> {
  const absolutePath = resolve(filePath);
  const code = await bundleFile(absolutePath);
  return renderFromCode(code, {
    ...options,
    _basePath: dirname(absolutePath),
  } as RenderOptionsInternal);
}

/// Render from pre-bundled ESM code string.
/// Handles the temp-file-and-import dance, then serializes and renders.
export async function renderFromCode(
  code: string,
  options?: RenderOptions,
): Promise<RenderResult> {
  const start = performance.now();
  const basePath = (options as RenderOptionsInternal)?._basePath;

  // Wrap the bundled code with a serialize re-export so it uses the same
  // @formepdf/react instance as the template (avoids dual-instance issues
  // when the renderer is bundled into a VS Code extension)
  const wrappedCode = code + `\nexport { serialize as __formeSerialize } from '@formepdf/react';\n`;

  // Write temp file in the source directory so Node resolves @formepdf/* from the user's node_modules
  const tmpDir = basePath ?? process.cwd();
  const tmpFile = join(tmpDir, `.forme-render-${Date.now()}.mjs`);
  await writeFile(tmpFile, wrappedCode);

  let mod: Record<string, unknown>;
  try {
    mod = await import(pathToFileURL(tmpFile).href);
  } finally {
    await unlink(tmpFile).catch(() => {});
  }

  // Use the user's serialize if available (same React instance as the template)
  const serializeFn = (typeof mod.__formeSerialize === 'function'
    ? mod.__formeSerialize
    : defaultSerialize) as (element: ReactElement) => unknown;

  const elementOpts: ResolveElementOptions = {};
  if (options?.data !== undefined) {
    elementOpts.data = options.data;
  } else if (options?.dataPath) {
    elementOpts.dataPath = options.dataPath;
  }

  const element = await resolveElement(mod, elementOpts);
  return renderFromElement(element, {
    pageSize: options?.pageSize,
    _basePath: basePath,
    _renderStart: start,
    _serialize: serializeFn,
  } as RenderFromElementInternalOptions);
}

/// Render from an already-resolved React element. Skips bundling entirely.
export async function renderFromElement(
  element: ReactElement,
  options?: Pick<RenderOptions, 'pageSize'>,
): Promise<RenderResult> {
  const start = (options as RenderFromElementInternalOptions)?._renderStart ?? performance.now();
  const basePath = (options as RenderFromElementInternalOptions)?._basePath;

  const serializeFn = (options as RenderFromElementInternalOptions)?._serialize ?? defaultSerialize;
  const doc = serializeFn(element) as unknown as Record<string, unknown>;

  if (options?.pageSize) {
    applyPageSizeOverride(doc, options.pageSize);
  }

  await resolveAllSources(doc, basePath);

  const { pdf, layout } = await renderPdfWithLayout(JSON.stringify(doc));
  const renderTimeMs = Math.round(performance.now() - start);

  return { pdf, layout, renderTimeMs };
}

function applyPageSizeOverride(
  doc: Record<string, unknown>,
  size: { width: number; height: number },
): void {
  const customSize = { Custom: { width: size.width, height: size.height } };

  if (doc.defaultPage && typeof doc.defaultPage === 'object') {
    (doc.defaultPage as Record<string, unknown>).size = customSize;
  }

  if (Array.isArray(doc.children)) {
    for (const child of doc.children) {
      if (child && typeof child === 'object' && child.kind && typeof child.kind === 'object') {
        const kind = child.kind as Record<string, unknown>;
        if (kind.type === 'Page' && kind.config && typeof kind.config === 'object') {
          (kind.config as Record<string, unknown>).size = customSize;
        }
      }
    }
  }
}

// Internal options for passing basePath and timing through the call chain
interface RenderOptionsInternal extends RenderOptions {
  _basePath?: string;
}

interface RenderFromElementInternalOptions extends Pick<RenderOptions, 'pageSize'> {
  _basePath?: string;
  _renderStart?: number;
  _serialize?: (element: ReactElement) => unknown;
}
