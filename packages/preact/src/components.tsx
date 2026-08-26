import type {
  DocumentProps,
  PageProps,
  ViewProps,
  TextProps,
  HeadingProps,
  OrderedListProps,
  UnorderedListProps,
  ListItemProps,
  StrongProps,
  EmProps,
  CodeProps,
  LinkProps,
  ImageProps,
  TableProps,
  RowProps,
  CellProps,
  FixedProps,
  SvgProps,
  QrCodeProps,
  BarcodeProps,
  CanvasProps,
  WatermarkProps,
  BarChartProps,
  LineChartProps,
  PieChartProps,
  AreaChartProps,
  DotPlotProps,
  TextFieldProps,
  CheckboxProps,
  DropdownProps,
  RadioButtonProps,
} from './types.js';

/**
 * Root document container. Must be the top-level element in every Forme document.
 *
 * All other Forme components must be descendants of `<Document>`.
 *
 * @param props.title - PDF metadata title
 * @param props.author - PDF metadata author
 * @param props.subject - PDF metadata subject
 * @param props.creator - PDF metadata creator application
 * @param props.style - Default style applied globally (fontFamily, fontSize, color, etc.)
 * @param props.children - Page or content elements
 *
 * @example
 * ```tsx
 * <Document title="Invoice" style={{ fontFamily: 'Inter', fontSize: 12 }}>
 *   <Text>Hello World</Text>
 * </Document>
 * ```
 */
export function Document(_props: DocumentProps): null {
  return null;
}
/** Stable marker for cross-version identity checks (survives minification). */
(Document as any).__formeType = 'Document';

/**
 * A page boundary with explicit size and margin configuration.
 *
 * When used, content inside each `<Page>` is laid out independently.
 * Without `<Page>`, content uses the document's default page config.
 *
 * @param props.size - Page size: `"A4"`, `"Letter"`, `"Legal"`, `"A3"`, `"A5"`, `"Tabloid"`, or `{ width, height }` in points
 * @param props.margin - Page margins as a uniform number or `{ top, right, bottom, left }`
 * @param props.children - Content elements for this page
 *
 * @example
 * ```tsx
 * <Document>
 *   <Page size="Letter" margin={72}>
 *     <Text>US Letter page with 1-inch margins</Text>
 *   </Page>
 * </Document>
 * ```
 */
export function Page(_props: PageProps): null {
  return null;
}

/**
 * A flex container, analogous to an HTML `<div>`.
 *
 * Defaults to `flexDirection: "column"`. Supports all flex properties
 * including `flexWrap`, `gap`, `justifyContent`, and `alignItems`.
 *
 * @param props.style - CSS-like style properties
 * @param props.wrap - Whether this container can break across pages (default: true). Set to `false` to keep children together.
 * @param props.children - Child elements
 *
 * @example
 * ```tsx
 * <View style={{ flexDirection: 'row', gap: 12, padding: 8 }}>
 *   <Text>Left</Text>
 *   <Text>Right</Text>
 * </View>
 * ```
 */
export function View(_props: ViewProps): null {
  return null;
}

/**
 * A text node. Children are flattened to a single string.
 *
 * Supports font properties, text alignment, color, and text transforms.
 * Nested `<Text>` children have their content concatenated.
 *
 * @param props.style - Typography and color styles
 * @param props.children - Text content (strings, numbers, or nested `<Text>`)
 *
 * @example
 * ```tsx
 * <Text style={{ fontSize: 24, fontWeight: 'bold', color: '#333' }}>
 *   Invoice Total: $1,234.00
 * </Text>
 * ```
 */
export function Text(_props: TextProps): null {
  return null;
}

/**
 * Semantic heading components. `<H1>` … `<H6>`. Each level has sensible
 * defaults (font size, weight, margin-top, margin-bottom) tuned for
 * document layout; pass `style` to override individual properties (your
 * values win on conflict).
 *
 * In tagged PDFs (`<Document tagged>` or `<Document pdfUa>`), headings
 * map to the corresponding `/H1` – `/H6` PDF structure elements
 * automatically — useful for screen-reader accessibility and for tools
 * that build a table of contents from the structure tree.
 *
 * Inline formatting components work inside: `<H1>Chapter <Em>1</Em></H1>`.
 *
 * @example
 * ```tsx
 * <H1>Annual Report</H1>
 * <H2 style={{ marginTop: 32 }}>Executive Summary</H2>
 * ```
 */
export function H1(_props: HeadingProps): null {
  return null;
}
export function H2(_props: HeadingProps): null {
  return null;
}
export function H3(_props: HeadingProps): null {
  return null;
}
export function H4(_props: HeadingProps): null {
  return null;
}
export function H5(_props: HeadingProps): null {
  return null;
}
export function H6(_props: HeadingProps): null {
  return null;
}

/**
 * An ordered (numbered) list. Children must be `<ListItem>` elements.
 *
 * Numbering continues across page breaks. The marker gutter is sized
 * automatically to fit the widest number the list will render.
 *
 * @param props.type   - Numbering style. `'decimal'` (default) | `'lower-alpha'`
 *                       | `'upper-alpha'` | `'lower-roman'` | `'upper-roman'`.
 * @param props.start  - Starting index. Defaults to `1`.
 * @param props.style  - Container style overrides.
 *
 * @example
 * ```tsx
 * <OrderedList type="lower-alpha">
 *   <ListItem>first</ListItem>
 *   <ListItem>second</ListItem>
 * </OrderedList>
 * ```
 */
export function OrderedList(_props: OrderedListProps): null {
  return null;
}

/**
 * An unordered (bulleted) list. Children must be `<ListItem>` elements.
 *
 * v1 ships with the standard "•" bullet glyph for `disc`, `circle`, and
 * `square` (proper distinct glyphs for circle and square are a follow-up).
 * `none` suppresses the marker entirely.
 *
 * @param props.marker - Bullet style. `'disc'` (default) | `'circle'`
 *                       | `'square'` | `'none'`.
 * @param props.style  - Container style overrides.
 *
 * @example
 * ```tsx
 * <UnorderedList>
 *   <ListItem>first</ListItem>
 *   <ListItem>second</ListItem>
 * </UnorderedList>
 * ```
 */
export function UnorderedList(_props: UnorderedListProps): null {
  return null;
}

/**
 * One item inside an `<OrderedList>` or `<UnorderedList>`. Content can
 * be text, inline-formatted runs, nested lists, or any Forme content
 * node — the marker is rendered to the left, content (including any
 * wrapped lines) aligns to the right of the marker gutter.
 */
export function ListItem(_props: ListItemProps): null {
  return null;
}

/**
 * Inline bold text. Use inside `<Text>` for runs of bold content.
 * Equivalent to wrapping content in a `<Text style={{ fontWeight: 700 }}>`
 * — just shorter. Composes with nested inline-formatting components
 * (e.g. `<Strong><Em>both</Em></Strong>` produces bold + italic).
 *
 * @param props.style - Optional style overrides (merged on top of the
 *   default `fontWeight: 700`)
 * @param props.children - Inline content (text, numbers, or other inline
 *   components like `<Em>`)
 *
 * @example
 * ```tsx
 * <Text>Read the <Strong>fine print</Strong> carefully.</Text>
 * ```
 */
export function Strong(_props: StrongProps): null {
  return null;
}

/**
 * Inline italic text. Use inside `<Text>` for runs of emphasized
 * content. Equivalent to `<Text style={{ fontStyle: 'italic' }}>`.
 *
 * @param props.style - Optional style overrides
 * @param props.children - Inline content
 *
 * @example
 * ```tsx
 * <Text>This is <Em>important</Em> to remember.</Text>
 * ```
 */
export function Em(_props: EmProps): null {
  return null;
}

/**
 * Inline monospace text with a subtle background — like Markdown's
 * `` `code` ``. Use inside `<Text>` for runs of inline code or paths.
 *
 * @param props.style - Optional style overrides (merged on top of the
 *   default font/background/padding)
 * @param props.children - Inline content
 *
 * @example
 * ```tsx
 * <Text>Run <Code>npm install</Code> to get started.</Text>
 * ```
 */
export function Code(_props: CodeProps): null {
  return null;
}

/**
 * Inline hyperlink. Renders blue + underlined and is clickable in the
 * output PDF. Use inside `<Text>` for inline links.
 *
 * @param props.href - The URL to link to (required)
 * @param props.style - Optional style overrides
 * @param props.children - Inline link content (the visible link text)
 *
 * @example
 * ```tsx
 * <Text>See the <Link href="https://docs.formepdf.com">docs</Link>.</Text>
 * ```
 */
export function Link(_props: LinkProps): null {
  return null;
}

/**
 * An image element. Supports JPEG, PNG, and WebP via data URIs or file paths.
 *
 * If only `width` or `height` is specified, the other dimension is calculated
 * from the image's aspect ratio.
 *
 * @param props.src - Image source:
 *   - `data:image/png;base64,...` (inline base64 data URI)
 *   - `./assets/logo.png` (relative to the template file)
 *   - `/absolute/path/logo.png` (absolute file path)
 * @param props.width - Display width in points
 * @param props.height - Display height in points
 * @param props.style - Additional style properties
 *
 * @example
 * ```tsx
 * <Image src="./assets/logo.png" width={200} />
 * ```
 */
export function Image(_props: ImageProps): null {
  return null;
}

/**
 * A table container. Children should be `<Row>` elements.
 *
 * Tables support automatic page breaks between rows and repeat header rows
 * on continuation pages.
 *
 * @param props.columns - Column width definitions: `{ fraction: 0.5 }`, `{ fixed: 100 }`, or `"auto"`
 * @param props.style - Style properties for the table container
 * @param props.children - `<Row>` elements
 *
 * @example
 * ```tsx
 * <Table columns={[{ width: { fraction: 0.6 } }, { width: { fraction: 0.4 } }]}>
 *   <Row header>
 *     <Cell><Text>Name</Text></Cell>
 *     <Cell><Text>Price</Text></Cell>
 *   </Row>
 *   <Row>
 *     <Cell><Text>Widget</Text></Cell>
 *     <Cell><Text>$10.00</Text></Cell>
 *   </Row>
 * </Table>
 * ```
 */
export function Table(_props: TableProps): null {
  return null;
}

/**
 * A table row. Must be a direct child of `<Table>`.
 *
 * Header rows (`header={true}`) are automatically repeated at the top of
 * each continuation page when a table spans multiple pages.
 *
 * @param props.header - Whether this is a header row that repeats on page breaks
 * @param props.style - Style properties (e.g., `backgroundColor` for alternating rows)
 * @param props.children - `<Cell>` elements
 *
 * @example
 * ```tsx
 * <Row header style={{ backgroundColor: '#333' }}>
 *   <Cell><Text style={{ color: '#fff' }}>Header</Text></Cell>
 * </Row>
 * ```
 */
export function Row(_props: RowProps): null {
  return null;
}

/**
 * A table cell inside a `<Row>`.
 *
 * @param props.colSpan - Number of columns this cell spans (default: 1)
 * @param props.rowSpan - Number of rows this cell spans (default: 1)
 * @param props.style - Style properties (padding, background, etc.)
 * @param props.children - Cell content (any Forme elements)
 *
 * @example
 * ```tsx
 * <Cell colSpan={2} style={{ padding: 8, backgroundColor: '#f5f5f5' }}>
 *   <Text>Spanning two columns</Text>
 * </Cell>
 * ```
 */
export function Cell(_props: CellProps): null {
  return null;
}

/**
 * A fixed element that repeats on every page as a header or footer.
 *
 * Fixed elements reduce the available content area on each page.
 * Use `{{pageNumber}}` and `{{totalPages}}` placeholders in text content
 * for automatic page numbering.
 *
 * @param props.position - `"header"` (top of page) or `"footer"` (bottom of page)
 * @param props.style - Style properties
 * @param props.children - Content to repeat on each page
 *
 * @example
 * ```tsx
 * <Fixed position="footer">
 *   <Text style={{ textAlign: 'center', fontSize: 10 }}>
 *     Page {'{{pageNumber}}'} of {'{{totalPages}}'}
 *   </Text>
 * </Fixed>
 * ```
 */
export function Fixed(_props: FixedProps): null {
  return null;
}

/**
 * An SVG element. Renders basic SVG shapes (rect, circle, ellipse, line, path, polygon, polyline) to PDF.
 *
 * @param props.width - Display width in points
 * @param props.height - Display height in points
 * @param props.viewBox - SVG viewBox string (e.g., "0 0 100 100")
 * @param props.content - SVG markup string (the inner content, not the outer <svg> tag)
 * @param props.style - Additional style properties (margin, etc.)
 *
 * @example
 * ```tsx
 * <Svg width={100} height={100} viewBox="0 0 100 100"
 *   content='<rect x="10" y="10" width="80" height="80" fill="blue"/>' />
 * ```
 */
export function Svg(_props: SvgProps): null {
  return null;
}

/**
 * A QR code rendered as crisp vector graphics in the PDF.
 *
 * @param props.data - The data to encode (URL, text, etc.)
 * @param props.size - Display size in points. QR codes are square. Defaults to available width.
 * @param props.color - QR code color. Default: black.
 * @param props.style - Style properties for the QR code container.
 *
 * @example
 * ```tsx
 * <QrCode data="https://formepdf.com" size={100} />
 * ```
 */
export function QrCode(_props: QrCodeProps): null {
  return null;
}

/**
 * A 1D barcode rendered as crisp vector rectangles in the PDF.
 *
 * @param props.data - The data to encode
 * @param props.format - Barcode format: "Code128" (default), "Code39", "EAN13", "EAN8", "Codabar"
 * @param props.width - Width in points. Defaults to available width.
 * @param props.height - Height in points. Default: 60.
 * @param props.color - Bar color. Default: black.
 * @param props.style - Style properties for the barcode container.
 *
 * @example
 * ```tsx
 * <Barcode data="ABC-123" format="Code128" width={200} height={50} />
 * ```
 */
export function Barcode(_props: BarcodeProps): null {
  return null;
}

/**
 * A canvas drawing primitive for arbitrary vector graphics.
 *
 * The `draw` callback receives a `CanvasContext` that records drawing operations.
 * These are serialized to JSON and rendered as native PDF vector commands.
 *
 * @param props.width - Canvas width in points
 * @param props.height - Canvas height in points
 * @param props.draw - Drawing callback that receives a `CanvasContext`
 * @param props.style - Additional style properties
 *
 * @example
 * ```tsx
 * <Canvas width={200} height={100} draw={(ctx) => {
 *   ctx.setFillColor(0, 0, 255);
 *   ctx.rect(10, 10, 180, 80);
 *   ctx.fill();
 * }} />
 * ```
 */
export function Canvas(_props: CanvasProps): null {
  return null;
}

/**
 * A watermark rendered as rotated text behind all page content.
 *
 * Appears on every page. Use `rgba()` colors for translucent watermarks.
 *
 * @param props.text - Watermark text (e.g. "DRAFT", "CONFIDENTIAL")
 * @param props.fontSize - Font size in points. Default: 60.
 * @param props.color - Text color. Use `rgba(0,0,0,0.1)` for subtle watermarks. Default: "rgba(0,0,0,0.1)".
 * @param props.angle - Rotation angle in degrees. Default: -45.
 * @param props.style - Additional style properties (fontFamily, etc.)
 *
 * @example
 * ```tsx
 * <Page size="A4" margin={40}>
 *   <Watermark text="DRAFT" fontSize={60} color="rgba(0,0,0,0.1)" angle={-45} />
 *   <Text>Document content here</Text>
 * </Page>
 * ```
 */
export function Watermark(_props: WatermarkProps): null {
  return null;
}

/**
 * An explicit page break. Content after this element starts on a new page.
 *
 * @example
 * ```tsx
 * <Text>Page 1 content</Text>
 * <PageBreak />
 * <Text>Page 2 content</Text>
 * ```
 */
export function PageBreak(_props: object): null {
  return null;
}

/**
 * A bar chart rendered as native vector graphics in the PDF.
 *
 * @param props.data - Array of { label, value, color? } data points
 * @param props.width - Chart width in points
 * @param props.height - Chart height in points
 * @param props.color - Default bar color (hex). Default: "#1a365d"
 * @param props.showLabels - Show X-axis labels. Default: true
 * @param props.showValues - Show value labels above bars. Default: false
 * @param props.showGrid - Show horizontal grid lines. Default: false
 * @param props.title - Chart title
 */
export function BarChart(_props: BarChartProps): null {
  return null;
}

/**
 * A line chart rendered as native vector graphics in the PDF.
 *
 * @param props.series - Array of { name, data: number[], color? } series
 * @param props.labels - X-axis labels
 * @param props.width - Chart width in points
 * @param props.height - Chart height in points
 * @param props.showPoints - Show dots at data points. Default: false
 * @param props.showGrid - Show horizontal grid lines. Default: false
 * @param props.title - Chart title
 */
export function LineChart(_props: LineChartProps): null {
  return null;
}

/**
 * A pie chart rendered as native vector graphics in the PDF.
 *
 * @param props.data - Array of { label, value, color? } data points
 * @param props.width - Chart width in points
 * @param props.height - Chart height in points
 * @param props.donut - Render as donut chart. Default: false
 * @param props.showLegend - Show legend. Default: false
 * @param props.title - Chart title
 */
export function PieChart(_props: PieChartProps): null {
  return null;
}

/**
 * An area chart rendered as native vector graphics in the PDF.
 *
 * @param props.series - Array of { name, data: number[], color? } series
 * @param props.labels - X-axis labels
 * @param props.width - Chart width in points
 * @param props.height - Chart height in points
 * @param props.showGrid - Show horizontal grid lines. Default: false
 * @param props.title - Chart title
 */
export function AreaChart(_props: AreaChartProps): null {
  return null;
}

/**
 * A dot plot (scatter plot) rendered as native vector graphics in the PDF.
 *
 * @param props.groups - Array of { name, color?, data: [x, y][] } groups
 * @param props.width - Chart width in points
 * @param props.height - Chart height in points
 * @param props.dotSize - Dot radius. Default: 4
 * @param props.showLegend - Show legend. Default: false
 */
export function DotPlot(_props: DotPlotProps): null {
  return null;
}

/**
 * An interactive text input field (PDF AcroForm widget).
 *
 * @param props.name - Unique field name (PDF field identifier)
 * @param props.width - Field width in points
 * @param props.height - Field height in points. Default: 24
 * @param props.value - Pre-filled value
 * @param props.placeholder - Placeholder text shown when empty
 * @param props.multiline - Allow multi-line input. Default: false
 * @param props.password - Mask input. Default: false
 * @param props.readOnly - Prevent editing. Default: false
 * @param props.maxLength - Maximum character count
 * @param props.fontSize - Font size in points. Default: 12
 *
 * @example
 * ```tsx
 * <TextField name="full_name" width={200} placeholder="Enter your name" />
 * ```
 */
export function TextField(_props: TextFieldProps): null {
  return null;
}

/**
 * An interactive checkbox (PDF AcroForm widget).
 *
 * @param props.name - Unique field name
 * @param props.checked - Whether checked. Default: false
 * @param props.width - Display width in points. Default: 14
 * @param props.height - Display height in points. Default: 14
 * @param props.readOnly - Prevent editing. Default: false
 *
 * @example
 * ```tsx
 * <Checkbox name="agree_terms" checked={false} />
 * ```
 */
export function Checkbox(_props: CheckboxProps): null {
  return null;
}

/**
 * An interactive dropdown / combo box (PDF AcroForm widget).
 *
 * @param props.name - Unique field name
 * @param props.options - List of selectable options
 * @param props.value - Pre-selected value
 * @param props.width - Field width in points
 * @param props.height - Field height in points. Default: 24
 * @param props.readOnly - Prevent editing. Default: false
 * @param props.fontSize - Font size in points. Default: 12
 *
 * @example
 * ```tsx
 * <Dropdown name="country" options={["US", "UK", "CA"]} width={200} />
 * ```
 */
export function Dropdown(_props: DropdownProps): null {
  return null;
}

/**
 * An interactive radio button (PDF AcroForm widget).
 * Multiple RadioButtons with the same `name` form a mutually exclusive group.
 *
 * @param props.name - Group name (shared by all buttons in the group)
 * @param props.value - The value this button represents
 * @param props.checked - Whether selected. Default: false
 * @param props.width - Display width in points. Default: 14
 * @param props.height - Display height in points. Default: 14
 * @param props.readOnly - Prevent editing. Default: false
 *
 * @example
 * ```tsx
 * <RadioButton name="plan" value="free" checked />
 * <RadioButton name="plan" value="pro" />
 * <RadioButton name="plan" value="team" />
 * ```
 */
export function RadioButton(_props: RadioButtonProps): null {
  return null;
}
