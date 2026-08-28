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

export function renderHtml(html: string, options?: RenderHtmlOptions): RenderHtmlResult;
