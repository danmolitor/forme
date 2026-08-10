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

/** A single CSS Grid track size */
export type GridTrackSize =
  | number            // Fixed size in points
  | `${number}fr`     // Fractional unit (e.g. "1fr", "2fr")
  | 'auto'            // Content-sized
  | { min: GridTrackSize; max: GridTrackSize };  // MinMax

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
  display?: 'flex' | 'grid';
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

  // CSS Grid
  /** Column track definitions. String shorthand: `"1fr 2fr 200"` or array of track sizes. */
  gridTemplateColumns?: string | GridTrackSize[];
  /** Row track definitions. String shorthand: `"auto 1fr"` or array of track sizes. */
  gridTemplateRows?: string | GridTrackSize[];
  /** Default size for auto-generated rows. */
  gridAutoRows?: GridTrackSize;
  /** Default size for auto-generated columns. */
  gridAutoColumns?: GridTrackSize;
  /** Grid column start line (1-based). */
  gridColumnStart?: number;
  /** Grid column end line (1-based). */
  gridColumnEnd?: number;
  /** Grid row start line (1-based). */
  gridRowStart?: number;
  /** Grid row end line (1-based). */
  gridRowEnd?: number;
  /** Number of columns to span. */
  gridColumnSpan?: number;
  /** Number of rows to span. */
  gridRowSpan?: number;

  // Box model
  padding?: number | string | number[] | Edges;
  paddingTop?: number;
  paddingRight?: number;
  paddingBottom?: number;
  paddingLeft?: number;
  paddingHorizontal?: number;
  paddingVertical?: number;
  margin?: number | string | number[] | Edges;
  marginTop?: number | 'auto';
  marginRight?: number | 'auto';
  marginBottom?: number | 'auto';
  marginLeft?: number | 'auto';
  marginHorizontal?: number | 'auto';
  marginVertical?: number | 'auto';

  // Typography
  fontSize?: number;
  fontFamily?: string;
  fontWeight?: number | 'normal' | 'bold';
  fontStyle?: 'normal' | 'italic' | 'oblique';
  lineHeight?: number;
  textAlign?: 'left' | 'center' | 'right' | 'justify';
  letterSpacing?: number;
  /** Extra width (points) added to each ASCII space — PDF Tw operator.
   *  Negative values tighten word gaps. When `textAlign: 'justify'` is
   *  set, the layout engine adds the computed slack-per-space on top. */
  wordSpacing?: number;
  /** Drop shadow painted behind the element. Object form is preferred;
   *  CSS-like string form `"offsetX offsetY blur color"` is also
   *  accepted. v1 paints a solid offset shadow only — `blur` is parsed
   *  for forward-compat but ignored. */
  boxShadow?:
    | string
    | { offsetX: number; offsetY: number; blur?: number; color: string };
  /**
   * Paint-only transform applied around the element's origin (default:
   * center). Layout flow is NOT affected — matches CSS `transform`.
   *
   * Accepts a CSS-like string with one or more space-separated ops:
   *   `"rotate(45deg)"`
   *   `"rotate(-15deg) scale(1.2)"`
   *   `"translate(10, -4) rotate(30deg)"`
   *
   * Supported ops: `rotate(<deg>deg)`, `scale(<x>[, <y>])`,
   * `translate(<x>[, <y>])`. Angles are degrees, lengths are points.
   */
  transform?: string;
  /**
   * Origin of the transform as fractions of the element box.
   * `"50% 50%"` (default) = center, `"0% 0%"` = top-left, `"100% 0%"`
   * = top-right, etc. Also accepts a tuple `[x, y]` where each is 0–1.
   */
  transformOrigin?: string | [number, number];
  textDecoration?: 'none' | 'underline' | 'line-through';
  textTransform?: 'none' | 'uppercase' | 'lowercase' | 'capitalize';
  hyphens?: 'none' | 'manual' | 'auto';
  /** Language tag (BCP 47, e.g. "en-US", "de"). Controls hyphenation dictionary. */
  lang?: string;
  /** Text direction for BiDi support (Arabic, Hebrew). */
  direction?: 'ltr' | 'rtl' | 'auto';
  /** Text overflow behavior: 'wrap' (default), 'ellipsis' (truncate with ...), 'clip' (truncate). */
  textOverflow?: 'wrap' | 'ellipsis' | 'clip';
  /** Line breaking algorithm: 'optimal' (Knuth-Plass, default) or 'greedy'. */
  lineBreaking?: 'optimal' | 'greedy';
  /** Overflow behavior: 'visible' (default) or 'hidden' (clips children to bounds). */
  overflow?: 'visible' | 'hidden';

  // Visual
  color?: string;
  backgroundColor?: string;
  /**
   * CSS background. Supports:
   *   - solid colors (`"#1e293b"`, `"rgba(...)"`) — equivalent to backgroundColor
   *   - 2-stop linear gradients: `"linear-gradient(180deg, #fff 0%, #000 100%)"`
   *   - 2-stop radial gradients: `"radial-gradient(circle, #fff 0%, #000 100%)"`
   *
   * v1 supports exactly 2 color stops; gradients with 3+ stops fall back to
   * the first and last stop. Multi-stop support is planned via PDF Type 3
   * stitching functions in a follow-up.
   */
  background?: string;
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

export interface CertificationConfig {
  /** PEM-encoded X.509 certificate. */
  certificatePem: string;
  /** PEM-encoded RSA private key (PKCS#8). */
  privateKeyPem: string;
  /** Reason for certification (e.g. "Approved"). */
  reason?: string;
  /** Location of certification (e.g. "New York, NY"). */
  location?: string;
  /** Contact info for the certifier. */
  contact?: string;
  /** Whether to show a visible signature annotation on the page. */
  visible?: boolean;
  /** X coordinate in points for visible signature. */
  x?: number;
  /** Y coordinate in points for visible signature. */
  y?: number;
  /** Width in points for visible signature. */
  width?: number;
  /** Height in points for visible signature. */
  height?: number;
}

/** @deprecated Use CertificationConfig */
export type SignatureConfig = CertificationConfig;

export interface ColumnDef {
  width: { fraction: number } | { fixed: number } | 'auto';
}

export type BarcodeFormat = 'Code128' | 'Code39' | 'EAN13' | 'EAN8' | 'Codabar';

/** Data point for bar and pie charts. */
export interface ChartDataPoint {
  label: string;
  value: number;
  /** Optional per-item color (hex string). */
  color?: string;
}

/** @deprecated Use ChartDataPoint instead (color is now optional on ChartDataPoint). */
export interface PieDataPoint {
  label: string;
  value: number;
  color: string;
}

/** A data series for multi-series line and area charts. */
export interface ChartSeries {
  name: string;
  data: number[];
  /** Optional series color (hex string). */
  color?: string;
}

/** A group of (x, y) data points for dot plots. */
export interface DotPlotGroup {
  name: string;
  /** Optional group color (hex string). */
  color?: string;
  data: [number, number][];
}

/** Canvas drawing context for the draw callback. */
export interface CanvasContext {
  moveTo(x: number, y: number): void;
  lineTo(x: number, y: number): void;
  bezierCurveTo(cp1x: number, cp1y: number, cp2x: number, cp2y: number, x: number, y: number): void;
  quadraticCurveTo(cpx: number, cpy: number, x: number, y: number): void;
  closePath(): void;
  rect(x: number, y: number, w: number, h: number): void;
  circle(cx: number, cy: number, r: number): void;
  ellipse(cx: number, cy: number, rx: number, ry: number): void;
  arc(cx: number, cy: number, r: number, startAngle: number, endAngle: number, counterclockwise?: boolean): void;
  /** Convenience: draws a stroked line from (x1,y1) to (x2,y2). */
  line(x1: number, y1: number, x2: number, y2: number): void;
  stroke(): void;
  fill(): void;
  fillAndStroke(): void;
  setFillColor(r: number, g: number, b: number): void;
  setStrokeColor(r: number, g: number, b: number): void;
  setLineWidth(w: number): void;
  setLineCap(cap: number): void;
  setLineJoin(join: number): void;
  save(): void;
  restore(): void;
}

/** A single canvas drawing operation (serialized to JSON). */
export type CanvasOp =
  | { op: 'MoveTo'; x: number; y: number }
  | { op: 'LineTo'; x: number; y: number }
  | { op: 'BezierCurveTo'; cp1x: number; cp1y: number; cp2x: number; cp2y: number; x: number; y: number }
  | { op: 'QuadraticCurveTo'; cpx: number; cpy: number; x: number; y: number }
  | { op: 'ClosePath' }
  | { op: 'Rect'; x: number; y: number; width: number; height: number }
  | { op: 'Circle'; cx: number; cy: number; r: number }
  | { op: 'Ellipse'; cx: number; cy: number; rx: number; ry: number }
  | { op: 'Arc'; cx: number; cy: number; r: number; start_angle: number; end_angle: number; counterclockwise: boolean }
  | { op: 'Stroke' }
  | { op: 'Fill' }
  | { op: 'FillAndStroke' }
  | { op: 'SetFillColor'; r: number; g: number; b: number }
  | { op: 'SetStrokeColor'; r: number; g: number; b: number }
  | { op: 'SetLineWidth'; width: number }
  | { op: 'SetLineCap'; cap: number }
  | { op: 'SetLineJoin'; join: number }
  | { op: 'Save' }
  | { op: 'Restore' };

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
  defaultStyle?: FormeStyle;
  fonts?: FormeFont[];
  tagged?: boolean;
  pdfa?: '2a' | '2b';
  pdfUa?: boolean;
  flattenForms?: boolean;
  certification?: CertificationConfig;
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
  backgroundImage?: string;
  backgroundOpacity?: number;
  backgroundSize?: 'fill' | 'cover' | 'contain';
  backgroundPosition?: 'center' | 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';
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

export interface FormeBoxShadow {
  offsetX: number;
  offsetY: number;
  blur?: number;
  color: { r: number; g: number; b: number; a: number };
}

export interface FormeGradientStop {
  position: number;
  color: { r: number; g: number; b: number; a: number };
}

export type FormeBackground =
  | { type: 'color'; value: { r: number; g: number; b: number; a: number } }
  | { type: 'linear'; angleDeg: number; stops: FormeGradientStop[] }
  | { type: 'radial'; stops: FormeGradientStop[] };

/** Margin edges that support auto values (for centering). */
export interface FormeMarginEdges {
  top: number | 'auto';
  right: number | 'auto';
  bottom: number | 'auto';
  left: number | 'auto';
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
  | { type: 'Heading'; level: number; content: string; href?: string; runs?: TextRun[] }
  | { type: 'List'; ordered: boolean; marker_type: FormeListMarkerType; start: number }
  | { type: 'ListItem' }
  | { type: 'Image'; src: string; width?: number; height?: number }
  | { type: 'Table'; columns: FormeColumnDef[] }
  | { type: 'TableRow'; is_header: boolean }
  | { type: 'TableCell'; col_span: number; row_span: number }
  | { type: 'Fixed'; position: 'Header' | 'Footer' }
  | { type: 'Svg'; width: number; height: number; view_box?: string; content: string }
  | { type: 'QrCode'; data: string; size?: number }
  | { type: 'Barcode'; data: string; format: BarcodeFormat; width?: number; height: number }
  | { type: 'Canvas'; width: number; height: number; operations: CanvasOp[] }
  | { type: 'BarChart'; data: ChartDataPoint[]; width: number; height: number; color?: string; show_labels: boolean; show_values: boolean; show_grid: boolean; title?: string }
  | { type: 'LineChart'; series: ChartSeries[]; labels: string[]; width: number; height: number; show_points: boolean; show_grid: boolean; title?: string }
  | { type: 'PieChart'; data: ChartDataPoint[]; width: number; height: number; donut: boolean; show_legend: boolean; title?: string }
  | { type: 'AreaChart'; series: ChartSeries[]; labels: string[]; width: number; height: number; show_grid: boolean; title?: string }
  | { type: 'DotPlot'; groups: DotPlotGroup[]; width: number; height: number; x_min?: number; x_max?: number; y_min?: number; y_max?: number; x_label?: string; y_label?: string; show_legend: boolean; dot_size: number }
  | { type: 'Watermark'; text: string; font_size: number; angle: number }
  | { type: 'TextField'; name: string; width: number; height: number; value?: string; placeholder?: string; multiline: boolean; password: boolean; read_only: boolean; max_length?: number; font_size: number }
  | { type: 'Checkbox'; name: string; width: number; height: number; checked: boolean; read_only: boolean }
  | { type: 'Dropdown'; name: string; options: string[]; width: number; height: number; value?: string; read_only: boolean; font_size: number }
  | { type: 'RadioButton'; name: string; value: string; width: number; height: number; checked: boolean; read_only: boolean }
  | { type: 'PageBreak' };

/**
 * Marker style for an ordered or unordered list. Matches CSS
 * `list-style-type` for the supported values.
 */
export type ListMarker =
  | 'disc'
  | 'circle'
  | 'square'
  | 'none'
  | 'decimal'
  | 'lower-alpha'
  | 'upper-alpha'
  | 'lower-roman'
  | 'upper-roman';

/** Engine wire format for list marker type — matches the Rust enum's
 *  serde camelCase output. */
export type FormeListMarkerType =
  | 'disc'
  | 'circle'
  | 'square'
  | 'none'
  | 'decimal'
  | 'lowerAlpha'
  | 'upperAlpha'
  | 'lowerRoman'
  | 'upperRoman';

/** A single transform operation in the engine wire format. The discriminator
 *  is `type` matching the engine's serde tag. */
export type FormeTransformOp =
  | { type: 'rotate'; deg: number }
  | { type: 'scale'; x: number; y: number }
  | { type: 'translate'; x: number; y: number };

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

/** Grid track size in Forme JSON format (matches Rust GridTrackSize enum) */
export type FormeGridTrackSize =
  | { Pt: number }
  | { Fr: number }
  | 'Auto'
  | { MinMax: [FormeGridTrackSize, FormeGridTrackSize] };

/** Grid placement in Forme JSON format */
export interface FormeGridPlacement {
  columnStart?: number;
  columnEnd?: number;
  rowStart?: number;
  rowEnd?: number;
  columnSpan?: number;
  rowSpan?: number;
}

/** Style in the Forme JSON format (camelCase field names, PascalCase enum values) */
export interface FormeStyle {
  display?: string;
  width?: FormeDimension;
  height?: FormeDimension;
  minWidth?: FormeDimension;
  minHeight?: FormeDimension;
  maxWidth?: FormeDimension;
  maxHeight?: FormeDimension;
  padding?: FormeEdges;
  margin?: FormeMarginEdges;
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
  gridTemplateColumns?: FormeGridTrackSize[];
  gridTemplateRows?: FormeGridTrackSize[];
  gridAutoRows?: FormeGridTrackSize;
  gridAutoColumns?: FormeGridTrackSize;
  gridPlacement?: FormeGridPlacement;
  fontFamily?: string;
  fontSize?: number;
  fontWeight?: number;
  fontStyle?: string;
  lineHeight?: number;
  textAlign?: string;
  letterSpacing?: number;
  wordSpacing?: number;
  boxShadow?: FormeBoxShadow;
  transform?: FormeTransformOp[];
  transformOrigin?: [number, number];
  textDecoration?: string;
  textTransform?: string;
  hyphens?: string;
  lang?: string;
  direction?: string;
  textOverflow?: string;
  lineBreaking?: string;
  overflow?: string;
  color?: FormeColor;
  backgroundColor?: FormeColor;
  background?: FormeBackground;
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
