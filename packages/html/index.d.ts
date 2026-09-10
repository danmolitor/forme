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
  /** Emit a tagged PDF (structure tree). Implied by `pdfUa`. */
  tagged?: boolean;
  /**
   * Emit a PDF/UA-1 conforming file: structure tree, metadata, embedded fonts.
   * Register a metric-compatible font (`@formepdf/fonts-standard`) via `fonts`,
   * set `lang`, and give informational `<img>`s `alt` text. Missing pieces are
   * reported in `warnings`, never silently dropped.
   */
  pdfUa?: boolean;
  /**
   * Emit a PDF/UA-2 (ISO 14289-2:2024) conforming file — the PDF 2.0
   * successor to `pdfUa`. Implies PDF 2.0 output (every font must be
   * embedded) and tagging with the 2.0 structure namespace. Contradicts
   * `pdfUa` and the 2x/3x `pdfA` levels; composes with `pdfA: "4"`.
   */
  pdfUa2?: boolean;
  /** Document language for PDF/UA (`/Lang`), e.g. `"en"` or `"en-US"`. Falls
   *  back to the `<html lang>` attribute, then `"en"` with a warning. */
  lang?: string;
  /**
   * PDF/A conformance level: `"2b"` (visual), `"2u"` (+ Unicode mapping), or
   * `"2a"` (+ full tagging); `"4"` is ISO 19005-4:2020 over PDF 2.0 (no
   * a/b/u split — and no accessibility claim; compose with `pdfUa2` for
   * that). Needs an embeddable font registered via `fonts`
   * (`@formepdf/fonts-standard`). Composes with `pdfUa` (2x/3x) or
   * `pdfUa2` ("4") — a file can be both archival and accessible.
   * "4f" is not offered here: it requires an embedded file and the HTML
   * path has no attachments option (the engine refuses it by name).
   */
  pdfA?: '2b' | '2u' | '2a' | '3b' | '3u' | '3a' | '4';
  /**
   * Opt-in post-render content audit: after layout, the engine verifies the
   * pages against the input and reports dropped, fully off-page, invisible
   * (colour == background), or clipped-to-nothing content as
   * `render defect:` entries in `warnings`. Costs nothing when omitted.
   */
  auditContent?: boolean;
}

export interface RenderHtmlResult {
  pdf: Uint8Array;
  /** Everything outside the documented subset, named — never silent. */
  warnings: string[];
  /**
   * Number of layout passes the render took. 1 for the common case; 2–3 only
   * when a page-number placeholder (`{{pageNumber}}`/`{{totalPages}}`, or CSS
   * `counter(page)`/`counter(pages)`) needs its reserved width corrected.
   */
  passes: number;
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

/**
 * No-op on the Node entry (the nodejs WASM target self-initializes). Present
 * so all three entries share one surface; only `@formepdf/html/worker` needs
 * a real `init(module)`.
 */
export function init(): Promise<void>;

export function renderHtml(html: string, options?: RenderHtmlOptions): RenderHtmlResult;

/**
 * Render to PDF plus `LayoutInfo`. The layout is identical in shape to a
 * core JSX render's, so downstream tooling consumes both paths uniformly.
 */
export function renderHtmlWithLayout(
  html: string,
  options?: RenderHtmlOptions,
): RenderHtmlLayoutResult;
