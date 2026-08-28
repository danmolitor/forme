import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { renderHtmlWithLayout, type RenderHtmlOptions } from '@formepdf/html';
import type { LayoutInfo } from '@formepdf/core';
import type { RenderResult } from './render.js';

/// Options for the HTML input path. Unlike JSX there is no bundling, data
/// binding, or asset resolution step — the source string goes straight to
/// the engine, which owns pagination via the document's own `@page` rules.
export interface HtmlRenderOptions {
  /// Overrides the document's `@page size` (print-dialog precedence).
  pageSize?: RenderHtmlOptions['pageSize'];
  /// Uniform page margin in points; overrides `@page` margins.
  pageMargin?: number;
  /// Extra CSS appended after the document's own stylesheets.
  css?: string;
}

/// Render an HTML string to PDF + LayoutInfo, in the same RenderResult shape
/// as the JSX path so the preview panel's format-agnostic tail is reused
/// verbatim. `warnings` carries the mapper's unsupported-subset notices.
export function renderHtmlFromSource(
  source: string,
  options?: HtmlRenderOptions,
): RenderResult {
  const start = performance.now();
  const { pdf, layout, warnings } = renderHtmlWithLayout(source, options ?? {});
  const renderTimeMs = Math.round(performance.now() - start);
  return { pdf, layout: layout as unknown as LayoutInfo, renderTimeMs, warnings };
}

/// Read an `.html` file and render it. Mirrors `renderFromFile` for JSX.
export async function renderHtmlFromFile(
  filePath: string,
  options?: HtmlRenderOptions,
): Promise<RenderResult> {
  const source = await readFile(resolve(filePath), 'utf-8');
  return renderHtmlFromSource(source, options);
}
