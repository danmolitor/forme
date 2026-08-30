// Components
export { default as Document } from './components/Document.vue';
export { default as Page } from './components/Page.vue';
export { default as View } from './components/View.vue';
export { default as Text } from './components/Text.vue';
export { default as H1 } from './components/H1.vue';
export { default as H2 } from './components/H2.vue';
export { default as H3 } from './components/H3.vue';
export { default as H4 } from './components/H4.vue';
export { default as H5 } from './components/H5.vue';
export { default as H6 } from './components/H6.vue';
export { default as OrderedList } from './components/OrderedList.vue';
export { default as UnorderedList } from './components/UnorderedList.vue';
export { default as ListItem } from './components/ListItem.vue';
export { default as Strong } from './components/Strong.vue';
export { default as Em } from './components/Em.vue';
export { default as Code } from './components/Code.vue';
export { default as Link } from './components/Link.vue';
export { default as Table } from './components/Table.vue';
export { default as Row } from './components/Row.vue';
export { default as Cell } from './components/Cell.vue';
export { default as Fixed } from './components/Fixed.vue';
export { default as Image } from './components/Image.vue';
export { default as Svg } from './components/Svg.vue';
export { default as QrCode } from './components/QrCode.vue';
export { default as Barcode } from './components/Barcode.vue';
export { default as Canvas } from './components/Canvas.vue';
export { default as Watermark } from './components/Watermark.vue';
export { default as PageBreak } from './components/PageBreak.vue';
export { default as BarChart } from './components/BarChart.vue';
export { default as LineChart } from './components/LineChart.vue';
export { default as PieChart } from './components/PieChart.vue';
export { default as AreaChart } from './components/AreaChart.vue';
export { default as DotPlot } from './components/DotPlot.vue';
export { default as TextField } from './components/TextField.vue';
export { default as Checkbox } from './components/Checkbox.vue';
export { default as Dropdown } from './components/Dropdown.vue';
export { default as RadioButton } from './components/RadioButton.vue';

// Page-number placeholders
export { PAGE_NUMBER, TOTAL_PAGES } from './constants.js';

// Font registration (the shared store singleton — registrations are
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
  Style, TextRun, ListMarker, ColumnDef, BarcodeFormat, ChartDataPoint, ChartSeries,
  DotPlotGroup, BarChartProps, LineChartProps, PieChartProps, AreaChartProps, DotPlotProps,
  CanvasContext, CanvasOp, GridTrackSize, Edges, Corners, EdgeColors, CertificationConfig,
  SignatureConfig, FormeDocument, FormeFont, FormeNode, FormeNodeKind, FormeStyle,
  FormeMetadata, FormePageConfig, FormePageSize, FormeEdges, FormeColumnDef, FormeColumnWidth,
  FormeDimension, FormeColor, FormeEdgeValues, FormeCornerValues, FormeGridTrackSize,
  FormeGridPlacement, FormeListMarkerType, FormeTransformOp,
} from '@formepdf/shared';
