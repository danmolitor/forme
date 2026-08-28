export interface RenderHtmlOptions {
  /** Overrides the document's `@page size` (print-dialog precedence). */
  pageSize?: 'A4' | 'A3' | 'A5' | 'Letter' | 'Legal' | 'Tabloid';
  /** Uniform page margin in points; overrides `@page` margins. */
  pageMargin?: number;
  /** Extra CSS appended after the document's own stylesheets. */
  css?: string;
  /** TTF fonts registered under the family names templates reference. */
  fonts?: Array<{
    family: string;
    /** Raw TTF bytes, or a base64 string. */
    data: Uint8Array | string;
    /** CSS weight (400 regular, 700 bold). Default 400. */
    weight?: number;
    italic?: boolean;
  }>;
}

export interface RenderHtmlResult {
  pdf: Uint8Array;
  /** Everything outside the documented subset, named — never silent. */
  warnings: string[];
}

/**
 * The laid-out node tree. This is the same shape `@formepdf/core` emits for
 * JSX renders; re-declared here as an opaque structure so this package
 * carries no hard dependency on `@formepdf/core`'s types.
 */
export interface LayoutInfo {
  pages: unknown[];
}

export interface RenderHtmlLayoutResult extends RenderHtmlResult {
  /** The laid-out node tree — drives tree/inspector/overlay tooling. */
  layout: LayoutInfo;
}

export function renderHtml(html: string, options?: RenderHtmlOptions): RenderHtmlResult;

/**
 * Render to PDF plus `LayoutInfo`. The layout is identical in shape to a
 * core JSX render's, so downstream tooling consumes both paths uniformly.
 */
export function renderHtmlWithLayout(
  html: string,
  options?: RenderHtmlOptions,
): RenderHtmlLayoutResult;
