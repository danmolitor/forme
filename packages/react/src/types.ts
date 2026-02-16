import type { ReactNode } from 'react';

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
  flexGrow?: number;
  flexShrink?: number;
  flexBasis?: number | string;
  flexWrap?: 'nowrap' | 'wrap' | 'wrap-reverse';
  justifyContent?: 'flex-start' | 'flex-end' | 'center' | 'space-between' | 'space-around' | 'space-evenly';
  alignItems?: 'flex-start' | 'flex-end' | 'center' | 'stretch' | 'baseline';
  alignSelf?: 'flex-start' | 'flex-end' | 'center' | 'stretch' | 'baseline';
  gap?: number;
  rowGap?: number;
  columnGap?: number;

  // Box model
  padding?: number | Edges;
  margin?: number | Edges;

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

  // Visual
  color?: string;
  backgroundColor?: string;
  opacity?: number;
  borderWidth?: number | Edges;
  borderColor?: string | EdgeColors;
  borderRadius?: number | Corners;

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
  children?: ReactNode;
}

export interface PageProps {
  size?: 'A4' | 'A3' | 'A5' | 'Letter' | 'Legal' | 'Tabloid' | { width: number; height: number };
  margin?: number | Edges;
  children?: ReactNode;
}

export interface ViewProps {
  style?: Style;
  wrap?: boolean;
  children?: ReactNode;
}

export interface TextProps {
  style?: Style;
  children?: ReactNode;
}

export interface ImageProps {
  src: string;
  width?: number;
  height?: number;
  style?: Style;
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
  children?: ReactNode;
}

// ─── Forme JSON output types (match Rust serde format) ───────────────

export interface FormeDocument {
  children: FormeNode[];
  metadata: FormeMetadata;
  defaultPage: FormePageConfig;
}

export interface FormeMetadata {
  title?: string;
  author?: string;
  subject?: string;
  creator?: string;
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
}

export type FormeNodeKind =
  | { type: 'Page'; config: FormePageConfig }
  | { type: 'View' }
  | { type: 'Text'; content: string }
  | { type: 'Image'; src: string; width?: number; height?: number }
  | { type: 'Table'; columns: FormeColumnDef[] }
  | { type: 'TableRow'; is_header: boolean }
  | { type: 'TableCell'; col_span: number; row_span: number }
  | { type: 'Fixed'; position: 'Header' | 'Footer' }
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
  color?: FormeColor;
  backgroundColor?: FormeColor;
  opacity?: number;
  borderWidth?: FormeEdgeValues<number>;
  borderColor?: FormeEdgeValues<FormeColor>;
  borderRadius?: FormeCornerValues;
  wrap?: boolean;
  breakBefore?: boolean;
  minWidowLines?: number;
  minOrphanLines?: number;
}
