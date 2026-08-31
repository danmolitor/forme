import { readFile, writeFile, unlink } from 'node:fs/promises';
import { resolve, dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import type { ReactElement } from 'react';
import { renderPdfWithLayout, type LayoutInfo } from '@formepdf/core';
import { serialize as defaultSerialize } from '@formepdf/react';
import { bundleFile, bundleSource, detectJsxFlavor, type JsxFlavor } from './bundle.js';
import { resolveElement, type ResolveElementOptions } from './element.js';
import { resolveAllSources } from './resolve.js';
import { friendlyDependencyError } from './workspace.js';

/// The reconciler runtime + adapter each flavor imports at render time. The
/// wrapped bundle re-exports serialize/isValidElement from these so the
/// template and the serializer share one workspace-resolved instance.
const FLAVOR_IMPORTS: Record<JsxFlavor, { adapter: string; runtime: string; ext: string }> = {
  react: { adapter: '@formepdf/react', runtime: 'react', ext: '.tsx' },
  preact: { adapter: '@formepdf/preact', runtime: 'preact', ext: '.tsx' },
};

export interface RenderOptions {
  dataPath?: string;
  data?: unknown;
  pageSize?: { width: number; height: number };
}

/// A collision-proof temp filename. `Date.now()` alone collides when renders
/// run concurrently in the same directory (e.g. the cross-framework gate), so
/// a per-process counter and random suffix are mixed in.
let _tmpCounter = 0;
export function tempName(prefix: string, ext: string): string {
  return `${prefix}${Date.now()}-${process.pid}-${_tmpCounter++}-${Math.random().toString(36).slice(2, 8)}${ext}`;
}

export interface RenderResult {
  pdf: Uint8Array;
  layout: LayoutInfo;
  renderTimeMs: number;
  /// Unsupported-subset notices from the input path. The core/JSX pipeline
  /// surfaces none today, so it's always `[]` here; the HTML path populates
  /// it from the mapper. Same field, both paths.
  warnings: string[];
}

/// Full pipeline: bundle TSX file → resolve element → serialize → resolve assets → WASM render.
/// The reconciler flavor (react/preact) is detected from the file's import
/// signature, so a Preact template renders through the same dispatch.
export async function renderFromFile(
  filePath: string,
  options?: RenderOptions,
): Promise<RenderResult> {
  const absolutePath = resolve(filePath);
  const flavor = detectJsxFlavor(await readFile(absolutePath, 'utf-8'));
  const code = await bundleFile(absolutePath, flavor);
  return renderFromCode(code, {
    ...options,
    _basePath: dirname(absolutePath),
    _flavor: flavor,
  } as RenderOptionsInternal);
}

/// Full pipeline from source code string (e.g. an unsaved editor buffer).
/// `resolveDir` controls import resolution (typically the file's directory).
export async function renderFromSource(
  source: string,
  resolveDir: string,
  options?: RenderOptions & { sourcefile?: string },
): Promise<RenderResult> {
  const flavor = detectJsxFlavor(source);
  const code = await bundleSource(source, resolveDir, options?.sourcefile, flavor);
  return renderFromCode(code, {
    ...options,
    _basePath: resolveDir,
    _flavor: flavor,
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
  const flavor = (options as RenderOptionsInternal)?._flavor ?? 'react';
  const { adapter, runtime, ext } = FLAVOR_IMPORTS[flavor];

  // Wrap the bundled code with a serialize re-export so it uses the same
  // adapter instance as the template (avoids dual-instance issues when the
  // renderer is bundled into a VS Code extension). The adapter/runtime are
  // the reconciler flavor's — @formepdf/react + react, or @formepdf/preact +
  // preact — resolved from the user's workspace.
  const wrappedCode = code + `\nexport { serialize as __formeSerialize } from '${adapter}';\nexport { isValidElement as __formeIsValidElement } from '${runtime}';\n`;

  // Write temp file in the source directory so Node resolves @formepdf/* from the user's node_modules
  const tmpDir = basePath ?? process.cwd();
  const tmpFile = join(tmpDir, tempName('.forme-render-', '.mjs'));
  await writeFile(tmpFile, wrappedCode);

  let mod: Record<string, unknown>;
  try {
    mod = await import(pathToFileURL(tmpFile).href);
  } catch (err) {
    // A missing react/preact/@formepdf adapter in the user's workspace should
    // read as guidance, not an ESM-loader stack trace.
    throw friendlyDependencyError(err, ext);
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
  if (typeof mod.__formeIsValidElement === 'function') {
    elementOpts.isValidElement = mod.__formeIsValidElement as (obj: unknown) => boolean;
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

  return renderDocToResult(doc, { pageSize: options?.pageSize, basePath, startTime: start });
}

/// The format-agnostic render tail: page-size override → asset resolution →
/// WASM render. Every input that produces a Forme document lands here — JSX
/// via `renderFromElement`, `.svelte`/`.vue` via the SFC paths — so the
/// RenderResult shape and the (present-and-empty) warnings contract stay
/// identical across inputs. Reused, not copied.
export async function renderDocToResult(
  doc: Record<string, unknown>,
  options: { pageSize?: { width: number; height: number }; basePath?: string; startTime: number },
): Promise<RenderResult> {
  if (options.pageSize) {
    applyPageSizeOverride(doc, options.pageSize);
  }

  await resolveAllSources(doc, options.basePath);

  const { pdf, layout } = await renderPdfWithLayout(JSON.stringify(doc));
  const renderTimeMs = Math.round(performance.now() - options.startTime);

  // The core WASM binding surfaces no warnings today; keep the field present
  // and empty so both input paths share one RenderResult shape.
  return { pdf, layout, renderTimeMs, warnings: [] };
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
  _flavor?: JsxFlavor;
}

interface RenderFromElementInternalOptions extends Pick<RenderOptions, 'pageSize'> {
  _basePath?: string;
  _renderStart?: number;
  _serialize?: (element: ReactElement) => unknown;
}
