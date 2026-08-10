// Style mapping
export { mapStyle, mapDimension, parseColor, expandEdges, expandCorners, mapColumnWidth } from './style.js';

// Font registration
export { Font, mergeFonts } from './font.js';
export type { FontRegistration } from './font.js';

// Canvas recording
export { recordCanvasOperations } from './canvas.js';

// Semantic component constants (headings, inline formatting, lists)
export {
  STRONG_DEFAULTS,
  EM_DEFAULTS,
  CODE_DEFAULTS,
  LINK_DEFAULTS,
  HEADING_DEFAULTS,
  mapListMarker,
} from './semantics.js';

// Chart kind builders (shared camelCase-to-snake_case prop mapping)
export {
  buildBarChartKind,
  buildLineChartKind,
  buildPieChartKind,
  buildAreaChartKind,
  buildDotPlotKind,
} from './charts.js';
export type {
  BarChartProps,
  LineChartProps,
  PieChartProps,
  AreaChartProps,
  DotPlotProps,
} from './charts.js';

// Types
export type {
  // Developer-facing
  Style,
  GridTrackSize,
  Edges,
  Corners,
  EdgeColors,
  CertificationConfig,
  SignatureConfig,
  ColumnDef,
  BarcodeFormat,
  ChartDataPoint,
  PieDataPoint,
  ChartSeries,
  DotPlotGroup,
  CanvasContext,
  CanvasOp,
  TextRun,
  ListMarker,
  // Forme JSON output
  FormeDocument,
  FormeFont,
  FormeNode,
  FormeNodeKind,
  FormeStyle,
  FormeMetadata,
  FormePageConfig,
  FormePageSize,
  FormeEdges,
  FormeBoxShadow,
  FormeGradientStop,
  FormeBackground,
  FormeMarginEdges,
  FormeColumnDef,
  FormeColumnWidth,
  FormeDimension,
  FormeColor,
  FormeEdgeValues,
  FormeCornerValues,
  FormeGridTrackSize,
  FormeGridPlacement,
  FormeListMarkerType,
  FormeTransformOp,
} from './types.js';
