// Components
export { Document, Page, View, Text, Image, Table, Row, Cell, Fixed, Svg, PageBreak } from './components.js';

// Serialization
export { serialize, mapStyle, mapDimension, parseColor, expandEdges, expandCorners } from './serialize.js';

// StyleSheet
export { StyleSheet } from './stylesheet.js';

// Font registration
export { Font } from './font.js';
export type { FontRegistration } from './font.js';

// Render functions
export { render, renderToObject } from './render.js';

// Types
export type {
  // Developer-facing
  Style,
  Edges,
  Corners,
  EdgeColors,
  DocumentProps,
  PageProps,
  ViewProps,
  TextProps,
  ImageProps,
  ColumnDef,
  TableProps,
  RowProps,
  CellProps,
  FixedProps,
  SvgProps,
  TextRun,
  // Forme JSON output
  FormeDocument,
  FormeFont,
  FormeNode,
  FormeNodeKind,
  FormeStyle,
  FormePageConfig,
  FormePageSize,
  FormeEdges,
  FormeMetadata,
  FormeColumnDef,
  FormeColumnWidth,
  FormeDimension,
  FormeColor,
  FormeEdgeValues,
  FormeCornerValues,
} from './types.js';
