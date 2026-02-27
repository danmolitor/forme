import type { ReactNode } from 'react';
import type { FontRegistration } from './font.js';

// ─── Developer-facing types ──────────────────────────────────────────

/** Edge values for padding, margin, borderWidth */
export interface Edges {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

/** Corner values for borderRadius */
export interface Corners {
  topLeft: number;
  topRight: number;
  bottomRight: number;
  bottomLeft: number;
}

/** Per-edge colors for borderColor */
export interface EdgeColors {
  top: string;
  right: string;
  bottom: string;
  left: string;
}

/** CSS-like style properties for Forme components */
export interface Style {
  // Layout
  width?: number | string;
  height?: number | string;
  minWidth?: number | string;
  minHeight?: number | string;
  maxWidth?: number | string;
  maxHeight?: number | string;
  flexDirection?: 'row' | 'column' | 'row-reverse' | 'column-reverse';
  flex?: number;
  flexGrow?: number;
  flexShrink?: number;
  flexBasis?: number | string;
  flexWrap?: 'nowrap' | 'wrap' | 'wrap-reverse';
  justifyContent?: 'flex-start' | 'flex-end' | 'center' | 'space-between' | 'space-around' | 'space-evenly';
  alignItems?: 'flex-start' | 'flex-end' | 'center' | 'stretch' | 'baseline';
  alignSelf?: 'flex-start' | 'flex-end' | 'center' | 'stretch' | 'baseline';
  alignContent?: 'flex-start' | 'flex-end' | 'center' | 'space-between' | 'space-around' | 'space-evenly' | 'stretch';
  gap?: number;
  rowGap?: number;
  columnGap?: number;

  // Box model
  padding?: number | string | number[] | Edges;
  paddingTop?: number;
  paddingRight?: number;
  paddingBottom?: number;
  paddingLeft?: number;
  paddingHorizontal?: number;
  paddingVertical?: number;
  margin?: number | string | number[] | Edges;
  marginTop?: number;
  marginRight?: number;
  marginBottom?: number;
  marginLeft?: number;
  marginHorizontal?: number;
  marginVertical?: number;

  // Typography
  fontSize?: number;
  fontFamily?: string;
  fontWeight?: number | 'normal' | 'bold';
  fontStyle?: 'normal' | 'italic' | 'oblique';
  lineHeight?: number;
  textAlign?: 'left' | 'center' | 'right' | 'justify';
  letterSpacing?: number;
  textDecoration?: 'none' | 'underline' | 'line-through';
  textTransform?: 'none' | 'uppercase' | 'lowercase' | 'capitalize';
  hyphens?: 'none' | 'manual' | 'auto';

  // Visual
  color?: string;
  backgroundColor?: string;
  opacity?: number;
  borderWidth?: number | Edges;
  borderTopWidth?: number;
  borderRightWidth?: number;
  borderBottomWidth?: number;
  borderLeftWidth?: number;
  borderColor?: string | EdgeColors;
  borderTopColor?: string;
  borderRightColor?: string;
  borderBottomColor?: string;
  borderLeftColor?: string;
  borderRadius?: number | Corners;
  borderTopLeftRadius?: number;
  borderTopRightRadius?: number;
  borderBottomRightRadius?: number;
  borderBottomLeftRadius?: number;

  // Border shorthands (CSS-like string parsing)
  /** CSS border shorthand, e.g. `"1px solid #000"` */
  border?: string;
  /** Per-side border shorthand: string parses as CSS, number sets width */
  borderTop?: string | number;
  /** Per-side border shorthand: string parses as CSS, number sets width */
  borderRight?: string | number;
  /** Per-side border shorthand: string parses as CSS, number sets width */
  borderBottom?: string | number;
  /** Per-side border shorthand: string parses as CSS, number sets width */
  borderLeft?: string | number;

  // Positioning
  position?: 'relative' | 'absolute';
  top?: number;
  right?: number;
  bottom?: number;
  left?: number;

  // Page behavior
  wrap?: boolean;
  breakBefore?: boolean;
  minWidowLines?: number;
  minOrphanLines?: number;
}

// ─── Component prop types ────────────────────────────────────────────

export interface DocumentProps {
  title?: string;
  author?: string;
  subject?: string;
  creator?: string;
  /** Document language (BCP 47 tag, e.g. "en-US"). Emitted as /Lang in the PDF Catalog. */
  lang?: string;
  fonts?: FontRegistration[];
  children?: ReactNode;
}

export interface PageProps {
  size?: 'A4' | 'A3' | 'A5' | 'Letter' | 'Legal' | 'Tabloid' | { width: number; height: number };
  margin?: number | string | number[] | Edges;
  children?: ReactNode;
}

export interface ViewProps {
  style?: Style;
  wrap?: boolean;
  bookmark?: string;
  href?: string;
  children?: ReactNode;
}

export interface TextProps {
  style?: Style;
  href?: string;
  bookmark?: string;
  children?: ReactNode;
}

export interface ImageProps {
  src: string;
  width?: number;
  height?: number;
  style?: Style;
  /** Optional hyperlink URL — makes the image clickable. */
  href?: string;
  /** Alt text for accessibility. */
  alt?: string;
}

export interface ColumnDef {
  width: { fraction: number } | { fixed: number } | 'auto';
}

export interface TableProps {
  columns?: ColumnDef[];
  style?: Style;
  children?: ReactNode;
}

export interface RowProps {
  header?: boolean;
  style?: Style;
  children?: ReactNode;
}

export interface CellProps {
  colSpan?: number;
  rowSpan?: number;
  style?: Style;
  children?: ReactNode;
}

export interface FixedProps {
  position: 'header' | 'footer';
  style?: Style;
  bookmark?: string;
  children?: ReactNode;
}

export interface SvgProps {
  width: number;
  height: number;
  viewBox?: string;
  content: string;
  style?: Style;
  /** Optional hyperlink URL — makes the SVG clickable. */
  href?: string;
  /** Alt text for accessibility. */
  alt?: string;
}

/** A styled text segment within a <Text> element */
export interface TextRun {
  content: string;
  style?: FormeStyle;
  href?: string;
}

// ─── Forme JSON output types (match Rust serde format) ───────────────

export interface FormeFont {
  family: string;
  src: string | Uint8Array;
  weight: number;
  italic: boolean;
}

export interface FormeDocument {
  children: FormeNode[];
  metadata: FormeMetadata;
  defaultPage: FormePageConfig;
  fonts?: FormeFont[];
}

export interface FormeMetadata {
  title?: string;
  author?: string;
  subject?: string;
  creator?: string;
  lang?: string;
}

export interface FormePageConfig {
  size: FormePageSize;
  margin: FormeEdges;
  wrap: boolean;
}

export type FormePageSize =
  | 'A4' | 'A3' | 'A5' | 'Letter' | 'Legal' | 'Tabloid'
  | { Custom: { width: number; height: number } };

export interface FormeEdges {
  top: number;
  right: number;
  bottom: number;
  left: number;
}

export interface FormeNode {
  kind: FormeNodeKind;
  style: FormeStyle;
  children: FormeNode[];
  bookmark?: string;
  href?: string;
  alt?: string;
  sourceLocation?: { file: string; line: number; column: number };
}

export type FormeNodeKind =
  | { type: 'Page'; config: FormePageConfig }
  | { type: 'View' }
  | { type: 'Text'; content: string; href?: string; runs?: TextRun[] }
  | { type: 'Image'; src: string; width?: number; height?: number }
  | { type: 'Table'; columns: FormeColumnDef[] }
  | { type: 'TableRow'; is_header: boolean }
  | { type: 'TableCell'; col_span: number; row_span: number }
  | { type: 'Fixed'; position: 'Header' | 'Footer' }
  | { type: 'Svg'; width: number; height: number; view_box?: string; content: string }
  | { type: 'PageBreak' };

export interface FormeColumnDef {
  width: FormeColumnWidth;
}

export type FormeColumnWidth =
  | { Fraction: number }
  | { Fixed: number }
  | 'Auto';

export type FormeDimension =
  | { Pt: number }
  | { Percent: number }
  | 'Auto';

export interface FormeColor {
  r: number;
  g: number;
  b: number;
  a: number;
}

export interface FormeEdgeValues<T> {
  top: T;
  right: T;
  bottom: T;
  left: T;
}

export interface FormeCornerValues {
  top_left: number;
  top_right: number;
  bottom_right: number;
  bottom_left: number;
}

/** Style in the Forme JSON format (camelCase field names, PascalCase enum values) */
export interface FormeStyle {
  width?: FormeDimension;
  height?: FormeDimension;
  minWidth?: FormeDimension;
  minHeight?: FormeDimension;
  maxWidth?: FormeDimension;
  maxHeight?: FormeDimension;
  padding?: FormeEdges;
  margin?: FormeEdges;
  flexDirection?: string;
  justifyContent?: string;
  alignItems?: string;
  alignSelf?: string;
  alignContent?: string;
  flexWrap?: string;
  flexGrow?: number;
  flexShrink?: number;
  flexBasis?: FormeDimension;
  gap?: number;
  rowGap?: number;
  columnGap?: number;
  fontFamily?: string;
  fontSize?: number;
  fontWeight?: number;
  fontStyle?: string;
  lineHeight?: number;
  textAlign?: string;
  letterSpacing?: number;
  textDecoration?: string;
  textTransform?: string;
  hyphens?: string;
  color?: FormeColor;
  backgroundColor?: FormeColor;
  opacity?: number;
  borderWidth?: FormeEdgeValues<number>;
  borderColor?: FormeEdgeValues<FormeColor>;
  borderRadius?: FormeCornerValues;
  position?: string;
  top?: number;
  right?: number;
  bottom?: number;
  left?: number;
  wrap?: boolean;
  breakBefore?: boolean;
  minWidowLines?: number;
  minOrphanLines?: number;
}
