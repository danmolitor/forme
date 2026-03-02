// Components
export { Document, Page, View, Text, Image, Table, Row, Cell, Fixed, Svg, QrCode, Canvas, Watermark, PageBreak } from './components.js';
export { BarChart, LineChart, PieChart } from './charts.js';

// Serialization
export { serialize, serializeTemplate, mapStyle, mapDimension, parseColor, expandEdges, expandCorners } from './serialize.js';

// StyleSheet
export { StyleSheet } from './stylesheet.js';

// Font registration
export { Font } from './font.js';
export type { FontRegistration } from './font.js';

// Template compilation
export { createDataProxy, isRefMarker, isEachMarker, isExprMarker } from './template-proxy.js';
export { expr } from './expr.js';

// Render functions
export { render, renderToObject } from './render.js';

// Types
export type {
  // Developer-facing
  Style,
  GridTrackSize,
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
  QrCodeProps,
  CanvasProps,
  CanvasContext,
  CanvasOp,
  WatermarkProps,
  ChartDataPoint,
  PieDataPoint,
  BarChartProps,
  LineChartProps,
  PieChartProps,
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
  FormeGridTrackSize,
  FormeGridPlacement,
} from './types.js';
