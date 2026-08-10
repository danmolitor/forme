// Components
export { default as Document } from './components/Document.svelte';
export { default as Page } from './components/Page.svelte';
export { default as View } from './components/View.svelte';
export { default as Text } from './components/Text.svelte';
export { default as H1 } from './components/H1.svelte';
export { default as H2 } from './components/H2.svelte';
export { default as H3 } from './components/H3.svelte';
export { default as H4 } from './components/H4.svelte';
export { default as H5 } from './components/H5.svelte';
export { default as H6 } from './components/H6.svelte';
export { default as OrderedList } from './components/OrderedList.svelte';
export { default as UnorderedList } from './components/UnorderedList.svelte';
export { default as ListItem } from './components/ListItem.svelte';
export { default as Strong } from './components/Strong.svelte';
export { default as Em } from './components/Em.svelte';
export { default as Code } from './components/Code.svelte';
export { default as Link } from './components/Link.svelte';
export { default as Table } from './components/Table.svelte';
export { default as Row } from './components/Row.svelte';
export { default as Cell } from './components/Cell.svelte';
export { default as Fixed } from './components/Fixed.svelte';
export { default as Image } from './components/Image.svelte';
export { default as Svg } from './components/Svg.svelte';
export { default as QrCode } from './components/QrCode.svelte';
export { default as Barcode } from './components/Barcode.svelte';
export { default as Canvas } from './components/Canvas.svelte';
export { default as Watermark } from './components/Watermark.svelte';
export { default as PageBreak } from './components/PageBreak.svelte';
export { default as BarChart } from './components/BarChart.svelte';
export { default as LineChart } from './components/LineChart.svelte';
export { default as PieChart } from './components/PieChart.svelte';
export { default as AreaChart } from './components/AreaChart.svelte';
export { default as DotPlot } from './components/DotPlot.svelte';
export { default as TextField } from './components/TextField.svelte';
export { default as Checkbox } from './components/Checkbox.svelte';
export { default as Dropdown } from './components/Dropdown.svelte';
export { default as RadioButton } from './components/RadioButton.svelte';

// Page-number placeholders
export { PAGE_NUMBER, TOTAL_PAGES } from './constants.js';

// Font registration (the shared store singleton - registrations are
// visible to every adapter in the process)
export { Font } from '@formepdf/shared';
export type { FontRegistration } from '@formepdf/shared';

// Serialization
export { serialize, render, renderToObject } from './serialize.js';
export type { SerializeOptions } from './serialize.js';
export { mapStyle, mapDimension, parseColor, expandEdges, expandCorners } from '@formepdf/shared';

// One-call PDF rendering (requires the optional @formepdf/core peer)
export { renderDocument, renderDocumentWithLayout } from './render-document.js';
export type { RenderDocumentOptions, RenderDocumentWithLayoutResult } from './render-document.js';

// StyleSheet
export { StyleSheet } from './stylesheet.js';

// Types
export type {
  // Developer-facing
  Style,
  TextRun,
  ListMarker,
  ColumnDef,
  BarcodeFormat,
  ChartDataPoint,
  ChartSeries,
  DotPlotGroup,
  BarChartProps,
  LineChartProps,
  PieChartProps,
  AreaChartProps,
  DotPlotProps,
  CanvasContext,
  CanvasOp,
  GridTrackSize,
  Edges,
  Corners,
  EdgeColors,
  CertificationConfig,
  SignatureConfig,
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
} from '@formepdf/shared';
