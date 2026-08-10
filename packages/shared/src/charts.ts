/**
 * Chart prop types and document-model kind builders shared by the
 * authoring adapters.
 *
 * The camelCase-to-snake_case prop mapping, including every default,
 * lives here so the adapters cannot drift: react and svelte both
 * serialize a chart by calling the same builder. Adapters remain
 * responsible for style mapping (`mapStyle`) and any adapter-specific
 * envelope (source locations, children).
 */

import type {
  ChartDataPoint,
  ChartSeries,
  DotPlotGroup,
  FormeNodeKind,
  Style,
} from './types.js';

export interface BarChartProps {
  width: number;
  height: number;
  data: ChartDataPoint[];
  /** Bar color. Default: "#1a365d". */
  color?: string;
  /** Show X-axis labels below bars. Default: true. */
  showLabels?: boolean;
  /** Show horizontal grid lines. Default: false. */
  showGrid?: boolean;
  /** Show value labels above bars. Default: false. */
  showValues?: boolean;
  /** Chart title. */
  title?: string;
  style?: Style;
}

export interface LineChartProps {
  width: number;
  height: number;
  /** Multi-series data. */
  series: ChartSeries[];
  /** X-axis labels. */
  labels: string[];
  /** Show dots at data points. Default: false. */
  showPoints?: boolean;
  /** Show horizontal grid lines. Default: false. */
  showGrid?: boolean;
  /** Chart title. */
  title?: string;
  style?: Style;
}

export interface PieChartProps {
  width: number;
  height: number;
  data: ChartDataPoint[];
  /** Render as donut chart. Default: false. */
  donut?: boolean;
  /** Show legend. Default: false. */
  showLegend?: boolean;
  /** Chart title. */
  title?: string;
  style?: Style;
}

export interface AreaChartProps {
  width: number;
  height: number;
  /** Multi-series data. */
  series: ChartSeries[];
  /** X-axis labels. */
  labels: string[];
  /** Show horizontal grid lines. Default: false. */
  showGrid?: boolean;
  /** Chart title. */
  title?: string;
  style?: Style;
}

export interface DotPlotProps {
  width: number;
  height: number;
  /** Groups of (x, y) data points. */
  groups: DotPlotGroup[];
  /** Minimum X value. Auto-computed if omitted. */
  xMin?: number;
  /** Maximum X value. Auto-computed if omitted. */
  xMax?: number;
  /** Minimum Y value. Auto-computed if omitted. */
  yMin?: number;
  /** Maximum Y value. Auto-computed if omitted. */
  yMax?: number;
  /** X-axis label. */
  xLabel?: string;
  /** Y-axis label. */
  yLabel?: string;
  /** Show legend. Default: false. */
  showLegend?: boolean;
  /** Dot radius in points. Default: 4. */
  dotSize?: number;
  style?: Style;
}

/** Copy data points so the kind owns plain data (per-datum color stays optional). */
function mapDataPoints(data: ChartDataPoint[]): ChartDataPoint[] {
  return data.map(d => ({ label: d.label, value: d.value, color: d.color }));
}

/** Build the document-model kind for a `<BarChart>`. */
export function buildBarChartKind(props: BarChartProps): FormeNodeKind {
  const kind: Extract<FormeNodeKind, { type: 'BarChart' }> = {
    type: 'BarChart',
    data: mapDataPoints(props.data),
    width: props.width,
    height: props.height,
    show_labels: props.showLabels ?? true,
    show_values: props.showValues ?? false,
    show_grid: props.showGrid ?? false,
  };
  if (props.color !== undefined) kind.color = props.color;
  if (props.title !== undefined) kind.title = props.title;
  return kind;
}

/** Build the document-model kind for a `<LineChart>`. */
export function buildLineChartKind(props: LineChartProps): FormeNodeKind {
  const kind: Extract<FormeNodeKind, { type: 'LineChart' }> = {
    type: 'LineChart',
    series: props.series.map(s => ({ name: s.name, data: s.data, color: s.color })),
    labels: props.labels,
    width: props.width,
    height: props.height,
    show_points: props.showPoints ?? false,
    show_grid: props.showGrid ?? false,
  };
  if (props.title !== undefined) kind.title = props.title;
  return kind;
}

/** Build the document-model kind for a `<PieChart>`. */
export function buildPieChartKind(props: PieChartProps): FormeNodeKind {
  const kind: Extract<FormeNodeKind, { type: 'PieChart' }> = {
    type: 'PieChart',
    data: mapDataPoints(props.data),
    width: props.width,
    height: props.height,
    donut: props.donut ?? false,
    show_legend: props.showLegend ?? false,
  };
  if (props.title !== undefined) kind.title = props.title;
  return kind;
}

/** Build the document-model kind for an `<AreaChart>`. */
export function buildAreaChartKind(props: AreaChartProps): FormeNodeKind {
  const kind: Extract<FormeNodeKind, { type: 'AreaChart' }> = {
    type: 'AreaChart',
    series: props.series.map(s => ({ name: s.name, data: s.data, color: s.color })),
    labels: props.labels,
    width: props.width,
    height: props.height,
    show_grid: props.showGrid ?? false,
  };
  if (props.title !== undefined) kind.title = props.title;
  return kind;
}

/** Build the document-model kind for a `<DotPlot>`. */
export function buildDotPlotKind(props: DotPlotProps): FormeNodeKind {
  const kind: Extract<FormeNodeKind, { type: 'DotPlot' }> = {
    type: 'DotPlot',
    groups: props.groups.map(g => ({ name: g.name, color: g.color, data: g.data })),
    width: props.width,
    height: props.height,
    show_legend: props.showLegend ?? false,
    dot_size: props.dotSize ?? 4,
  };
  if (props.xMin !== undefined) kind.x_min = props.xMin;
  if (props.xMax !== undefined) kind.x_max = props.xMax;
  if (props.yMin !== undefined) kind.y_min = props.yMin;
  if (props.yMax !== undefined) kind.y_max = props.yMax;
  if (props.xLabel !== undefined) kind.x_label = props.xLabel;
  if (props.yLabel !== undefined) kind.y_label = props.yLabel;
  return kind;
}
