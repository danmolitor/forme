import type {
  DocumentProps,
  PageProps,
  ViewProps,
  TextProps,
  ImageProps,
  TableProps,
  RowProps,
  CellProps,
  FixedProps,
  SvgProps,
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
 * @param props.children - Page or content elements
 *
 * @example
 * ```tsx
 * <Document title="Invoice" author="Acme Corp">
 *   <Text>Hello World</Text>
 * </Document>
 * ```
 */
export function Document(_props: DocumentProps): null {
  return null;
}

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
 * An image element. Supports JPEG and PNG via base64 data URIs or file paths.
 *
 * If only `width` or `height` is specified, the other dimension is calculated
 * from the image's aspect ratio.
 *
 * @param props.src - Image source: base64 data URI (`data:image/png;base64,...`) or file path
 * @param props.width - Display width in points
 * @param props.height - Display height in points
 * @param props.style - Additional style properties
 *
 * @example
 * ```tsx
 * <Image src="data:image/png;base64,..." width={200} />
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
