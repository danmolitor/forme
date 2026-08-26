// Node entry uses the wasm-pack `--target nodejs` build, which is a
// self-initializing CJS module: it `require('fs').readFileSync`s its
// own .wasm at import time, so callers don't need to await any init.
import { render_pdf as wasmRenderPdf } from '../pkg-node/forme.js';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import type { ReactElement } from 'react';

// ── Layout metadata types ──────────────────────────────────────────
//
// These describe the runtime shape that `renderDocumentWithLayout()`
// returns — NOT the JSX-authoring `Style` types from `@formepdf/react`.
// The layout tree does not mirror the JSX tree: several transforms run
// during layout. See the `ElementInfo` doc block below for the full
// list. There is a runtime-conformance test in this package
// (`tests/layout-shape.test.ts`) that renders a rich fixture and
// asserts every claim below; if these types drift from runtime again
// that test is the first thing that fails.

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

// ── Layout enum unions ─────────────────────────────────────────────
//
// The engine serializes style enums as PascalCase strings (Rust
// convention), NOT the CSS-style camelCase / kebab-case values you
// author with. Consumers writing `if (style.flexDirection === 'column')`
// silently fail — the runtime value is `'Column'`. The literal unions
// below make that a TypeScript compile error.
//
// Adding a new value to any of these enums (or to `ElementNodeType` /
// `ElementKind` below) is a minor-version change. Consumers doing
// exhaustive switches will get a TypeScript error and update; consumers
// narrowing on a specific known value are unaffected. That's the trade
// we want given how young and actively-growing FormePDF's node vocabulary
// is — the type strategy matches the reality.

export type ElementFlexDirection = 'Row' | 'Column' | 'RowReverse' | 'ColumnReverse';
export type ElementJustifyContent = 'FlexStart' | 'FlexEnd' | 'Center' | 'SpaceBetween' | 'SpaceAround' | 'SpaceEvenly';
export type ElementAlignItems = 'FlexStart' | 'FlexEnd' | 'Center' | 'Stretch' | 'Baseline';
export type ElementAlignContent = 'FlexStart' | 'FlexEnd' | 'Center' | 'SpaceBetween' | 'SpaceAround' | 'SpaceEvenly' | 'Stretch';
export type ElementFlexWrap = 'NoWrap' | 'Wrap' | 'WrapReverse';
export type ElementFontStyle = 'Normal' | 'Italic' | 'Oblique';
export type ElementTextAlign = 'Left' | 'Right' | 'Center' | 'Justify';
export type ElementTextDecoration = 'None' | 'Underline' | 'LineThrough';
export type ElementTextTransform = 'None' | 'Uppercase' | 'Lowercase' | 'Capitalize';
export type ElementOverflow = 'Visible' | 'Hidden';
export type ElementPosition = 'Relative' | 'Absolute';

/**
 * Semantic role of a layout node. Note the specific transforms below
 * (also documented on `ElementInfo`) — this union describes what the
 * runtime actually emits, not what the JSX author wrote:
 *
 * - Six discrete heading tags (`H1`–`H6`) — NO generic `'Heading'`
 * - `TableRow` / `TableCell` — NO `Table` wrapper (unwrapped by layout)
 * - `List` + `ListItem` + `Lbl` — from `<OrderedList>` / `<UnorderedList>`
 * - `FixedHeader` / `FixedFooter` — NO single `Fixed` (split by position)
 * - `TextLine` — leaf lines under `Text` blocks (holds `textContent`)
 * - Inline `<Strong>`/`<Em>`/`<Code>`/`<Link>` do not appear here;
 *   they contribute style runs within `TextLine`
 *
 * ### When adding a new value here
 *
 * Also add a `<Component>` that produces the new nodeType to
 * `RICH_FIXTURE` in `packages/core/tests/layout-shape.test.ts`.
 * A coverage tripwire in that file fails otherwise ("declared but
 * never rendered"), on the exact drift risk this whole file exists
 * to prevent.
 */
export type ElementNodeType =
  // Structural containers
  | 'View'
  | 'Text'
  | 'TextLine'
  // Semantic headings (discrete per tag)
  | 'H1' | 'H2' | 'H3' | 'H4' | 'H5' | 'H6'
  // Table primitives — no 'Table' wrapper node
  | 'TableRow'
  | 'TableCell'
  // Lists — <OrderedList> and <UnorderedList> both produce 'List'
  | 'List'
  | 'ListItem'
  | 'Lbl'
  // Fixed regions — <Fixed> splits by position into these two
  | 'FixedHeader'
  | 'FixedFooter'
  // Media
  | 'Image'
  | 'Svg'
  | 'QrCode'
  | 'Barcode'
  | 'Canvas'
  | 'Watermark'
  // Charts
  | 'BarChart'
  | 'LineChart'
  | 'PieChart'
  | 'AreaChart'
  | 'DotPlot'
  // Form fields
  | 'TextField'
  | 'Checkbox'
  | 'Dropdown'
  | 'RadioButton';

/**
 * Drawing kind for the node. Governs the PDF operator the serializer
 * emits; NOT the same as `nodeType`, which describes semantic role.
 */
export type ElementKind =
  | 'None'
  | 'Rect'
  | 'Text'
  | 'Image'
  | 'Svg'
  | 'QrCode'
  | 'Barcode'
  | 'Chart'
  | 'FormField'
  | 'Watermark';

export interface ElementStyleInfo {
  // Layout — flex + grid
  flexDirection: ElementFlexDirection;
  justifyContent: ElementJustifyContent;
  alignItems: ElementAlignItems;
  alignContent: ElementAlignContent;
  flexWrap: ElementFlexWrap;
  flexGrow: number;
  flexShrink: number;
  gap: number;
  columnGap: number;
  rowGap: number;

  // Positioning
  position: ElementPosition;
  /** Offset from parent, in points. Only meaningful when `position === 'Absolute'`. */
  top?: number;
  right?: number;
  bottom?: number;
  left?: number;

  // Box model
  margin: EdgeValues<number>;
  padding: EdgeValues<number>;
  borderWidth: EdgeValues<number>;
  borderColor: EdgeValues<Color>;
  borderRadius: CornerValues;
  /**
   * Explicit `style.width` from the source. May be a number (points) or
   * a stringified value (e.g. percentage) — the layout engine formats
   * some dimension variants as strings in the JSON output. Prefer the
   * top-level `ElementInfo.width` for the resolved rendered width.
   */
  width?: number | string;
  /** Explicit `style.height`. See `width` for shape notes. */
  height?: number | string;

  // Typography
  fontFamily: string;
  fontSize: number;
  fontWeight: number;
  fontStyle: ElementFontStyle;
  lineHeight: number;
  letterSpacing: number;
  textAlign: ElementTextAlign;
  textDecoration: ElementTextDecoration;
  textTransform: ElementTextTransform;

  // Colors + visibility
  color: Color;
  backgroundColor: Color | null;
  opacity: number;
  overflow: ElementOverflow;

  // Page break control
  breakBefore: boolean;
  breakable: boolean;
  minOrphanLines: number;
  minWidowLines: number;
}

/**
 * A single node in the layout tree returned by
 * `renderDocumentWithLayout()`. The tree does NOT mirror the JSX
 * source — several transforms happen during layout:
 *
 * - `<Table>` is unwrapped. Its `<Row>` children appear as sibling
 *   `TableRow` nodes at the containing page/View level. There is no
 *   `Table` wrapper node.
 * - `<OrderedList>` and `<UnorderedList>` both produce a `List` node
 *   containing `ListItem` children. Each `ListItem` has a `Lbl` child
 *   (the marker "1." / "•") followed by the item's own content children.
 * - `<Fixed position="header">` produces a `FixedHeader` nodeType and
 *   `<Fixed position="footer">` produces `FixedFooter`. There is no
 *   single `Fixed` nodeType.
 * - Headings render as six discrete `H1` … `H6` nodeTypes. There is
 *   no generic `Heading` nodeType with a `level` field.
 * - `<Text>` block content is split into `TextLine` leaf children.
 *   The actual text lives on `TextLine.textContent`; on non-`TextLine`
 *   nodes (including the parent `Text` block), `textContent` is `null`.
 * - Inline elements (`<Strong>`, `<Em>`, `<Code>`, `<Link>`) do NOT
 *   appear as their own nodes — they contribute style runs within
 *   `TextLine` leaves.
 * - `<PageBreak>` produces no node. It triggers a page break at
 *   layout time and is otherwise invisible.
 *
 * The runtime-conformance test in this package asserts every one of
 * these transforms explicitly. If it breaks, update this JSDoc first.
 */
export interface ElementInfo {
  x: number;
  y: number;
  width: number;
  height: number;

  kind: ElementKind;
  nodeType: ElementNodeType;

  style: ElementStyleInfo;
  children: ElementInfo[];

  /**
   * Rendered text for this line. Present ONLY on `TextLine` nodeType
   * leaves — every non-`TextLine` node (including the parent `Text`
   * block) emits `null` here at runtime. If you need the text of a
   * `Text` block, concatenate its `TextLine` children's `textContent`.
   */
  textContent?: string | null;

  /**
   * Source file / line / column of the JSX that produced this node.
   * Populated only when the render pipeline seeds
   * `globalThis.__formeSourceMap` — currently only the CLI dev server
   * does that. Production `renderDocument` / `renderDocumentWithLayout`
   * calls never populate this field.
   */
  sourceLocation?: { file: string; line: number; column: number };
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
