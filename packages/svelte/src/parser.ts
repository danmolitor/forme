/**
 * Markup parser: Svelte SSR output in, Forme document model out.
 *
 * The Forme Svelte components render namespaced placeholder tags
 * (`<forme-document>`, `<forme-page>`, `<forme-view>`, `<forme-text>`)
 * with a single `props` attribute holding the JSON-encoded component
 * props. This module parses that markup - the emitting components and
 * this parser are a matched pair inside this package. The placeholder
 * tag/attribute vocabulary is an INTERNAL contract, not a public
 * format: it may change in any release without notice.
 *
 * Whitespace is normalized to JSX-equivalent semantics so that a
 * `.svelte` template and its TSX counterpart serialize identically:
 * indentation and newlines never leak into text content, while
 * interior spaces and interpolation boundaries are preserved. The
 * Svelte compiler trims fragment edges and collapses inter-element
 * whitespace before we ever see the markup, so the remaining rules
 * are applied here (see `cleanJsxText`). Known divergences from JSX,
 * where the compiler destroys the distinction before serialization -
 * each matches how Svelte itself renders the template to the DOM:
 *
 * - Same-line leading/trailing spaces inside `<Text>` are trimmed
 *   (`<Text> a </Text>` → `"a"`; JSX keeps them).
 * - Interpolations split across lines join with a single space
 *   (JSX drops the whitespace-only literal between them entirely).
 * - Newlines inside interpolated *values* are treated like template
 *   text and collapse to a space.
 */

import { parseFragment } from 'parse5';
import type { DefaultTreeAdapterMap } from 'parse5';
import {
  buildAreaChartKind,
  buildBarChartKind,
  buildDotPlotKind,
  buildLineChartKind,
  buildPieChartKind,
  CODE_DEFAULTS,
  EM_DEFAULTS,
  expandEdges,
  Font,
  HEADING_DEFAULTS,
  LINK_DEFAULTS,
  mapColumnWidth,
  mapListMarker,
  mapStyle,
  mergeFonts,
  parseColor,
  STRONG_DEFAULTS,
} from '@formepdf/shared';
import type {
  BarcodeFormat,
  CanvasOp,
  CertificationConfig,
  ColumnDef,
  Edges,
  FontRegistration,
  FormeColumnDef,
  FormeDocument,
  FormeEdges,
  FormeNode,
  FormeNodeKind,
  FormePageConfig,
  FormePageSize,
  Style,
  TextRun,
} from '@formepdf/shared';
import { reviveBytesMarker } from './encode.js';

/** The slice of a chart placeholder's props the parser handles itself:
 *  every chart carries an optional style; the rest is chart-specific
 *  and typed by its shared kind builder. */
interface ChartPlaceholderProps {
  style?: Style;
}

type P5Node = DefaultTreeAdapterMap['childNode'];
type P5Element = DefaultTreeAdapterMap['element'];

type P5Text = DefaultTreeAdapterMap['textNode'];

function isElement(node: P5Node): node is P5Element {
  return 'tagName' in node;
}

function isText(node: P5Node): node is P5Text {
  return node.nodeName === '#text';
}

/**
 * Parse placeholder markup emitted by the Forme Svelte components into
 * a Forme JSON document object. The markup must contain a single
 * `<forme-document>` root element.
 */
interface DocumentProps {
  title?: string;
  author?: string;
  subject?: string;
  creator?: string;
  lang?: string;
  style?: Style;
  tagged?: boolean;
  pdfa?: '2a' | '2b';
  pdfUa?: boolean;
  certification?: CertificationConfig;
  /** @deprecated Use certification */
  signature?: CertificationConfig;
  fonts?: FontRegistration[];
}

export function parseMarkup(markup: string): FormeDocument {
  const fragment = parseFragment(markup);
  const root = findDocumentRoot(fragment.childNodes);

  const props = decodeProps(root, 'Document') as DocumentProps;

  // Separate Page children from content children
  const pageNodes: FormeNode[] = [];
  const contentNodes: FormeNode[] = [];
  for (const child of root.childNodes) {
    if (isElement(child) && child.tagName === 'forme-page') {
      pageNodes.push(parsePage(child));
    } else {
      const node = parseChild(child, 'Document');
      if (node) contentNodes.push(node);
    }
  }

  // If there are page nodes, use them. Otherwise the content stands alone.
  let children: FormeNode[];
  if (pageNodes.length > 0) {
    // Any loose content nodes get added to the last page's children
    if (contentNodes.length > 0) {
      const lastPage = pageNodes[pageNodes.length - 1];
      lastPage.children.push(...contentNodes);
    }
    children = pageNodes;
  } else {
    children = contentNodes;
  }

  const metadata: FormeDocument['metadata'] = {};
  if (props.title !== undefined) metadata.title = props.title;
  if (props.author !== undefined) metadata.author = props.author;
  if (props.subject !== undefined) metadata.subject = props.subject;
  if (props.creator !== undefined) metadata.creator = props.creator;
  if (props.lang !== undefined) metadata.lang = props.lang;

  // Merge global + document fonts (document fonts override on conflict)
  const mergedFonts = mergeFonts(Font.getRegistered(), props.fonts);

  const result: FormeDocument = {
    children,
    metadata,
    defaultPage: {
      size: 'A4',
      margin: { top: 54, right: 54, bottom: 54, left: 54 },
      wrap: true,
    },
  };

  if (props.style) result.defaultStyle = mapStyle(props.style);
  if (props.tagged !== undefined) result.tagged = props.tagged;
  if (props.pdfa !== undefined) result.pdfa = props.pdfa;
  if (props.pdfUa) result.pdfUa = true;
  const cert = props.certification ?? props.signature;
  if (cert) {
    if (props.signature && !props.certification) {
      console.warn('[Forme] The `signature` prop is deprecated. Use `certification` instead.');
    }
    result.certification = cert;
  }

  if (mergedFonts.length > 0) {
    result.fonts = mergedFonts;
  }

  return result;
}

// ─── Node dispatch ───────────────────────────────────────────────────

type ParentContext = 'Document' | 'Page' | 'View' | 'Table' | 'Row' | 'Cell' | 'Fixed' | null;

/**
 * Parse one markup node into a Forme node. Returns null for nodes that
 * produce no output (comments, i.e. Svelte SSR block anchors, and
 * whitespace-only text between elements).
 */
function parseChild(node: P5Node, parent: ParentContext): FormeNode | null {
  if (isElement(node)) {
    return parseElement(node, parent);
  }
  if (isText(node)) {
    // Loose text directly inside a container becomes an anonymous Text
    // node. Edges are trimmed: the Svelte compiler collapses the
    // whitespace around inter-element newlines to a single space, so
    // by the time the markup reaches the parser, indentation and
    // intentional edge spaces are indistinguishable.
    const content = cleanJsxText(node.value).replace(/^[ \t]+|[ \t]+$/g, '');
    if (content === '') return null;
    return {
      kind: { type: 'Text', content },
      style: {},
      children: [],
    };
  }
  return null;
}

function parseElement(element: P5Element, parent: ParentContext): FormeNode | null {
  switch (element.tagName) {
    case 'forme-view':
      return parseView(element);
    case 'forme-text':
      return parseText(element);
    case 'forme-h1':
      return parseHeading(element, 1);
    case 'forme-h2':
      return parseHeading(element, 2);
    case 'forme-h3':
      return parseHeading(element, 3);
    case 'forme-h4':
      return parseHeading(element, 4);
    case 'forme-h5':
      return parseHeading(element, 5);
    case 'forme-h6':
      return parseHeading(element, 6);
    case 'forme-ordered-list':
      return parseList(element, true);
    case 'forme-unordered-list':
      return parseList(element, false);
    case 'forme-list-item':
      return parseListItem(element);
    case 'forme-table':
      return parseTable(element);
    case 'forme-row':
      validateNesting('Row', parent);
      return parseRow(element);
    case 'forme-cell':
      validateNesting('Cell', parent);
      return parseCell(element);
    case 'forme-fixed':
      return parseFixed(element);
    case 'forme-image':
      return parseImage(element);
    case 'forme-svg':
      return parseSvg(element);
    case 'forme-qrcode':
      return parseQrCode(element);
    case 'forme-barcode':
      return parseBarcode(element);
    case 'forme-canvas':
      return parseCanvas(element);
    case 'forme-watermark':
      return parseWatermark(element);
    case 'forme-bar-chart':
      return parseChart(element, 'BarChart', buildBarChartKind);
    case 'forme-line-chart':
      return parseChart(element, 'LineChart', buildLineChartKind);
    case 'forme-pie-chart':
      return parseChart(element, 'PieChart', buildPieChartKind);
    case 'forme-area-chart':
      return parseChart(element, 'AreaChart', buildAreaChartKind);
    case 'forme-dot-plot':
      return parseChart(element, 'DotPlot', buildDotPlotKind);
    case 'forme-text-field':
      return parseTextField(element);
    case 'forme-checkbox':
      return parseCheckbox(element);
    case 'forme-dropdown':
      return parseDropdown(element);
    case 'forme-radio-button':
      return parseRadioButton(element);
    case 'forme-strong':
    case 'forme-em':
    case 'forme-code':
    case 'forme-link': {
      const name = INLINE_COMPONENT_NAMES[element.tagName];
      throw new Error(
        `<${name}> is an inline formatting component and must be inside a <Text> or heading. Wrap it: <Text><${name}>...</${name}></Text>`
      );
    }
    case 'forme-page-break':
      return { kind: { type: 'PageBreak' }, style: {}, children: [] };
    case 'forme-page':
      validateNesting('Page', parent);
      return parsePage(element);
    default:
      throw unknownElementError(element.tagName);
  }
}

// ─── Nesting validation ──────────────────────────────────────────────

const VALID_PARENTS: Record<string, { allowed: ParentContext[]; suggestion: string }> = {
  Page: {
    allowed: ['Document'],
    suggestion: '<Page> must be a direct child of <Document>.',
  },
  Row: {
    allowed: ['Table'],
    suggestion: '<Row> must be inside a <Table>. Wrap it: <Table><Row>...</Row></Table>',
  },
  Cell: {
    allowed: ['Row'],
    suggestion: '<Cell> must be inside a <Row>. Wrap it: <Row><Cell>...</Cell></Row>',
  },
};

function validateNesting(componentName: string, parent: ParentContext): void {
  const rule = VALID_PARENTS[componentName];
  if (!rule) return;
  if (parent !== null && !rule.allowed.includes(parent)) {
    throw new Error(
      `Invalid nesting: <${componentName}> found inside <${parent}>. ${rule.suggestion}`
    );
  }
}

// ─── Unknown elements ────────────────────────────────────────────────

const HTML_SUGGESTIONS: Record<string, string> = {
  div: 'View', span: 'Text', p: 'Text', h1: 'H1', h2: 'H2',
  h3: 'H3', h4: 'H4', h5: 'H5', h6: 'H6', img: 'Image',
  table: 'Table', tr: 'Row', td: 'Cell', ol: 'OrderedList',
  ul: 'UnorderedList', li: 'ListItem', strong: 'Strong', b: 'Strong',
  em: 'Em', i: 'Em', code: 'Code', a: 'Link',
};

function unknownElementError(tagName: string): Error {
  const suggestion = HTML_SUGGESTIONS[tagName];
  if (suggestion) {
    return new Error(`HTML element <${tagName}> is not supported. Use <${suggestion}> instead.`);
  }
  return new Error(`Unsupported element <${tagName}> in a Forme template.`);
}

function parseChildren(nodes: P5Node[], parent: ParentContext): FormeNode[] {
  const children: FormeNode[] = [];
  for (const node of nodes) {
    const child = parseChild(node, parent);
    if (child) children.push(child);
  }
  return children;
}

// ─── Page ────────────────────────────────────────────────────────────

interface PageProps {
  size?: string | { width: number; height: number };
  margin?: number | string | number[] | Edges;
  style?: Style;
  backgroundImage?: string;
  backgroundOpacity?: number;
  backgroundSize?: 'fill' | 'cover' | 'contain';
  backgroundPosition?: 'center' | 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';
}

function parsePage(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Page') as PageProps;

  let size: FormePageSize = 'A4';
  if (props.size !== undefined) {
    if (typeof props.size === 'string') {
      size = props.size as FormePageSize;
    } else {
      size = { Custom: { width: props.size.width, height: props.size.height } };
    }
  }

  let margin: FormeEdges = { top: 54, right: 54, bottom: 54, left: 54 };
  if (props.margin !== undefined) {
    margin = expandEdges(props.margin);
  }

  const config: FormePageConfig = { size, margin, wrap: true };
  if (props.backgroundImage !== undefined) config.backgroundImage = props.backgroundImage;
  if (props.backgroundOpacity !== undefined) config.backgroundOpacity = props.backgroundOpacity;
  if (props.backgroundSize !== undefined) config.backgroundSize = props.backgroundSize;
  if (props.backgroundPosition !== undefined) config.backgroundPosition = props.backgroundPosition;

  return {
    kind: { type: 'Page', config },
    style: props.style ? mapStyle(props.style) : {},
    children: parseChildren(element.childNodes, 'Page'),
  };
}

// ─── View ────────────────────────────────────────────────────────────

interface ViewProps {
  style?: Style;
  wrap?: boolean;
  bookmark?: string;
  href?: string;
}

function parseView(element: P5Element): FormeNode {
  const props = decodeProps(element, 'View') as ViewProps;
  const style = mapStyle(props.style);
  if (props.wrap !== undefined) {
    style.wrap = props.wrap;
  }

  const node: FormeNode = {
    kind: { type: 'View' },
    style,
    children: parseChildren(element.childNodes, 'View'),
  };
  if (props.bookmark) node.bookmark = props.bookmark;
  if (props.href) node.href = props.href;

  return node;
}

// ─── Text ────────────────────────────────────────────────────────────

interface TextProps {
  style?: Style;
  href?: string;
  bookmark?: string;
}

function parseText(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Text') as TextProps;

  const kind: FormeNode['kind'] = { type: 'Text', content: '' };
  const runs = textRunsOf(element.childNodes);
  if (runs) {
    kind.runs = runs;
  } else {
    kind.content = textContentOf(element.childNodes);
  }
  if (props.href) kind.href = props.href;

  const node: FormeNode = {
    kind,
    style: mapStyle(props.style),
    children: [],
  };
  if (props.bookmark) node.bookmark = props.bookmark;

  return node;
}

/** Inline formatting placeholder tags and their component-default
 *  styles. `forme-text` carries no defaults beyond the user's style -
 *  same table the react adapter keeps in `inlineDefaults()`. */
const INLINE_DEFAULTS: Record<string, Style> = {
  'forme-text': {},
  'forme-strong': STRONG_DEFAULTS,
  'forme-em': EM_DEFAULTS,
  'forme-code': CODE_DEFAULTS,
  'forme-link': LINK_DEFAULTS,
};

const INLINE_COMPONENT_NAMES: Record<string, string> = {
  'forme-strong': 'Strong',
  'forme-em': 'Em',
  'forme-code': 'Code',
  'forme-link': 'Link',
};

/**
 * Build styled text runs from a Text (or heading) element's children,
 * mirroring the react adapter's `buildTextRuns`: only used when at
 * least one direct child is an inline element (`<Text>`, `<Strong>`,
 * `<Em>`, `<Code>`, `<Link>`) - returns null otherwise. Descent
 * accumulates style and href: the outer accumulated style is overlaid
 * by each inline component's defaults, which are overlaid by the
 * user-supplied `style` on that element, so user style wins at every
 * level. `<Strong><Em>both</Em></Strong>` produces a single run with
 * bold + italic. Elements other than inline components are skipped but
 * still split the surrounding text into separate runs, as react's
 * per-child loop does.
 */
function textRunsOf(nodes: P5Node[]): TextRun[] | null {
  if (!nodes.some(n => isElement(n) && n.tagName in INLINE_DEFAULTS)) return null;
  return buildRuns(nodes, {}, undefined);
}

function buildRuns(nodes: P5Node[], accStyle: Style, accHref: string | undefined): TextRun[] {
  const runs: TextRun[] = [];
  let buffer = '';
  const flush = () => {
    const content = cleanJsxText(buffer);
    buffer = '';
    // JSX drops whitespace-only literals spanning lines but keeps
    // same-line spaces between spans; cleanJsxText reproduces that,
    // leaving only genuinely empty chunks to discard.
    if (content === '') return;
    const run: TextRun = { content };
    if (Object.keys(accStyle).length > 0) run.style = mapStyle(accStyle);
    if (accHref) run.href = accHref;
    runs.push(run);
  };

  for (const node of nodes) {
    if (isText(node)) {
      buffer += node.value;
    } else if (isElement(node)) {
      flush();
      const defaults = INLINE_DEFAULTS[node.tagName];
      if (defaults === undefined) continue; // unknown element inside <Text> - skip
      const props = decodeProps(node, INLINE_COMPONENT_NAMES[node.tagName] ?? 'Text') as TextProps;
      // Cascade: outer accumulated -> this component's defaults -> user
      // style. Later spreads win, so user style overrides defaults and
      // outer.
      const nextStyle: Style = { ...accStyle, ...defaults, ...(props.style || {}) };
      const nextHref = props.href ?? accHref;
      runs.push(...buildRuns(node.childNodes, nextStyle, nextHref));
    }
    // Comments (SSR block anchors) fall through: the text around them
    // stays one contiguous chunk.
  }
  flush();
  return runs;
}

// ─── Headings ────────────────────────────────────────────────────────

interface HeadingProps {
  style?: Style;
  href?: string;
  bookmark?: string;
}

/**
 * Parse an H1-H6 placeholder. Mirrors parseText for the children
 * machinery (mixed strings + inline-formatting components produce
 * runs) but emits the engine's `Heading { level }` node kind so the
 * tagged-PDF builder can pick up the semantic role. Default styles per
 * level are merged BEFORE the user's `style` prop, so user values win.
 */
function parseHeading(element: P5Element, level: 1 | 2 | 3 | 4 | 5 | 6): FormeNode {
  const props = decodeProps(element, `H${level}`) as HeadingProps;

  const kind: FormeNode['kind'] = { type: 'Heading', level, content: '' };
  const runs = textRunsOf(element.childNodes);
  if (runs) {
    kind.runs = runs;
  } else {
    kind.content = textContentOf(element.childNodes);
  }
  if (props.href) kind.href = props.href;

  // Defaults underlay; user style wins on conflicting keys.
  const mergedStyle: Style = { ...HEADING_DEFAULTS[level], ...(props.style || {}) };

  const node: FormeNode = {
    kind,
    style: mapStyle(mergedStyle),
    children: [],
  };
  if (props.bookmark) node.bookmark = props.bookmark;

  return node;
}

// ─── Lists ───────────────────────────────────────────────────────────

interface OrderedListProps {
  type?: string;
  start?: number;
  style?: Style;
  bookmark?: string;
}

interface UnorderedListProps {
  marker?: string;
  style?: Style;
  bookmark?: string;
}

interface ListItemProps {
  style?: Style;
}

function parseList(element: P5Element, ordered: boolean): FormeNode {
  const props = decodeProps(
    element,
    ordered ? 'OrderedList' : 'UnorderedList'
  ) as OrderedListProps & UnorderedListProps;

  const markerType = ordered
    ? mapListMarker(props.type, 'decimal')
    : mapListMarker(props.marker, 'disc');

  const start = typeof props.start === 'number' && props.start >= 1 ? props.start : 1;

  // Children must be ListItems - anything else is silently dropped to
  // keep the serializer tolerant, matching the react adapter. (Loose
  // whitespace and `{#if}` anchors between items are too common to be
  // a real error.)
  const children = element.childNodes
    .filter((c): c is P5Element => isElement(c) && c.tagName === 'forme-list-item')
    .map(c => parseListItem(c));

  const node: FormeNode = {
    kind: { type: 'List', ordered, marker_type: markerType, start },
    style: mapStyle(props.style),
    children,
  };
  if (props.bookmark) node.bookmark = props.bookmark;
  return node;
}

function parseListItem(element: P5Element): FormeNode {
  const props = decodeProps(element, 'ListItem') as ListItemProps;

  // ListItem content is laid out by the engine using layout_children,
  // so whatever the user put inside serializes as the node's children.
  // Loose strings auto-wrap in a Text node via parseChild, matching
  // the react adapter's convention.
  return {
    kind: { type: 'ListItem' },
    style: mapStyle(props.style),
    children: parseChildren(element.childNodes, null),
  };
}

/**
 * Extract the merged text content of a Text element's children.
 * Comments (Svelte SSR block anchors) vanish and the text around them
 * is treated as one contiguous chunk, so `{#if}`/`{#each}` boundaries
 * inside a `<Text>` never introduce breaks. Element children
 * contribute their own descendant text (react's flattenTextContent
 * recurses the same way).
 */
function textContentOf(nodes: P5Node[]): string {
  return cleanJsxText(rawTextOf(nodes));
}

function rawTextOf(nodes: P5Node[]): string {
  let merged = '';
  for (const node of nodes) {
    if (isText(node)) merged += node.value;
    else if (isElement(node)) merged += rawTextOf(node.childNodes);
  }
  return merged;
}

// ─── Table ───────────────────────────────────────────────────────────

interface TableProps {
  columns?: ColumnDef[];
  style?: Style;
}

function parseTable(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Table') as TableProps;
  const columns: FormeColumnDef[] = (props.columns ?? []).map(col => ({
    width: mapColumnWidth(col.width),
  }));

  return {
    kind: { type: 'Table', columns },
    style: mapStyle(props.style),
    children: parseChildren(element.childNodes, 'Table'),
  };
}

interface RowProps {
  header?: boolean;
  style?: Style;
}

function parseRow(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Row') as RowProps;

  return {
    kind: { type: 'TableRow', is_header: props.header ?? false },
    style: mapStyle(props.style),
    children: parseChildren(element.childNodes, 'Row'),
  };
}

interface CellProps {
  colSpan?: number;
  rowSpan?: number;
  style?: Style;
}

function parseCell(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Cell') as CellProps;

  return {
    kind: { type: 'TableCell', col_span: props.colSpan ?? 1, row_span: props.rowSpan ?? 1 },
    style: mapStyle(props.style),
    children: parseChildren(element.childNodes, 'Cell'),
  };
}

// ─── Fixed ───────────────────────────────────────────────────────────

interface FixedProps {
  position?: 'header' | 'footer';
  style?: Style;
  bookmark?: string;
}

function parseFixed(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Fixed') as FixedProps;
  // Mirrors react: anything other than 'header' falls back to Footer.
  const position = props.position === 'header' ? ('Header' as const) : ('Footer' as const);

  const node: FormeNode = {
    kind: { type: 'Fixed', position },
    style: mapStyle(props.style),
    children: parseChildren(element.childNodes, 'Fixed'),
  };
  if (props.bookmark) node.bookmark = props.bookmark;

  return node;
}

// ─── Media leaves ────────────────────────────────────────────────────

interface ImageProps {
  src: string;
  width?: number;
  height?: number;
  style?: Style;
  href?: string;
  alt?: string;
}

function parseImage(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Image') as ImageProps;

  const kind: FormeNodeKind = { type: 'Image', src: props.src };
  if (props.width !== undefined) kind.width = props.width;
  if (props.height !== undefined) kind.height = props.height;

  const node: FormeNode = {
    kind,
    style: mapStyle(props.style),
    children: [],
  };
  if (props.href) node.href = props.href;
  if (props.alt) node.alt = props.alt;
  return node;
}

interface SvgProps {
  width: number;
  height: number;
  viewBox?: string;
  content?: string;
  style?: Style;
  href?: string;
  alt?: string;
}

function parseSvg(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Svg') as SvgProps;

  const kind: FormeNodeKind = {
    type: 'Svg',
    width: props.width,
    height: props.height,
    content: props.content ?? '',
  };
  if (props.viewBox) kind.view_box = props.viewBox;

  const node: FormeNode = {
    kind,
    style: mapStyle(props.style),
    children: [],
  };
  if (props.href) node.href = props.href;
  if (props.alt) node.alt = props.alt;
  return node;
}

interface QrCodeProps {
  data: string;
  size?: number;
  color?: string;
  style?: Style;
}

function parseQrCode(element: P5Element): FormeNode {
  const props = decodeProps(element, 'QrCode') as QrCodeProps;

  const kind: FormeNodeKind = { type: 'QrCode', data: props.data };
  if (props.size !== undefined) kind.size = props.size;

  const style = mapStyle(props.style);
  if (props.color) style.color = parseColor(props.color);

  return { kind, style, children: [] };
}

interface BarcodeProps {
  data: string;
  format?: BarcodeFormat;
  width?: number;
  height?: number;
  color?: string;
  style?: Style;
}

function parseBarcode(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Barcode') as BarcodeProps;

  const kind: FormeNodeKind = {
    type: 'Barcode',
    data: props.data,
    format: props.format ?? 'Code128',
    height: props.height ?? 60,
  };
  if (props.width !== undefined) kind.width = props.width;

  const style = mapStyle(props.style);
  if (props.color) style.color = parseColor(props.color);

  return { kind, style, children: [] };
}

// ─── Vector extras ───────────────────────────────────────────────────

interface CanvasProps {
  width: number;
  height: number;
  /** Recorded by the emitting component, which executes the user's
   *  `draw` callback at emit time (the callback itself cannot survive
   *  the attribute round-trip). */
  operations: CanvasOp[];
  style?: Style;
}

function parseCanvas(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Canvas') as CanvasProps;

  return {
    kind: { type: 'Canvas', width: props.width, height: props.height, operations: props.operations },
    style: mapStyle(props.style),
    children: [],
  };
}

interface WatermarkProps {
  text: string;
  fontSize?: number;
  color?: string;
  angle?: number;
  style?: Style;
}

function parseWatermark(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Watermark') as WatermarkProps;
  const fontSize = props.fontSize ?? 60;
  const angle = props.angle ?? -45;

  // Mirrors react: the color's alpha channel becomes opacity (multiplied
  // with any style opacity); the stored color itself is fully opaque.
  const parsed = parseColor(props.color ?? 'rgba(0,0,0,0.1)');
  const style = mapStyle(props.style);
  style.color = { r: parsed.r, g: parsed.g, b: parsed.b, a: 1 };
  style.opacity = parsed.a * (style.opacity ?? 1);
  style.fontSize = fontSize;

  return {
    kind: { type: 'Watermark', text: props.text, font_size: fontSize, angle },
    style,
    children: [],
  };
}

// ─── Form fields ─────────────────────────────────────────────────────

interface TextFieldProps {
  name: string;
  value?: string;
  placeholder?: string;
  width: number;
  height?: number;
  multiline?: boolean;
  password?: boolean;
  readOnly?: boolean;
  maxLength?: number;
  fontSize?: number;
  style?: Style;
}

function parseTextField(element: P5Element): FormeNode {
  const props = decodeProps(element, 'TextField') as TextFieldProps;

  const kind: FormeNodeKind = {
    type: 'TextField',
    name: props.name,
    width: props.width,
    height: props.height ?? 24,
    multiline: props.multiline ?? false,
    password: props.password ?? false,
    read_only: props.readOnly ?? false,
    font_size: props.fontSize ?? 12,
  };
  if (props.value !== undefined) kind.value = props.value;
  if (props.placeholder !== undefined) kind.placeholder = props.placeholder;
  if (props.maxLength !== undefined) kind.max_length = props.maxLength;

  return { kind, style: mapStyle(props.style), children: [] };
}

interface CheckboxProps {
  name: string;
  checked?: boolean;
  width?: number;
  height?: number;
  readOnly?: boolean;
  style?: Style;
}

function parseCheckbox(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Checkbox') as CheckboxProps;

  return {
    kind: {
      type: 'Checkbox',
      name: props.name,
      checked: props.checked ?? false,
      width: props.width ?? 14,
      height: props.height ?? 14,
      read_only: props.readOnly ?? false,
    },
    style: mapStyle(props.style),
    children: [],
  };
}

interface DropdownProps {
  name: string;
  options: string[];
  value?: string;
  width: number;
  height?: number;
  readOnly?: boolean;
  fontSize?: number;
  style?: Style;
}

function parseDropdown(element: P5Element): FormeNode {
  const props = decodeProps(element, 'Dropdown') as DropdownProps;

  const kind: FormeNodeKind = {
    type: 'Dropdown',
    name: props.name,
    options: props.options,
    width: props.width,
    height: props.height ?? 24,
    read_only: props.readOnly ?? false,
    font_size: props.fontSize ?? 12,
  };
  if (props.value !== undefined) kind.value = props.value;

  return { kind, style: mapStyle(props.style), children: [] };
}

interface RadioButtonProps {
  name: string;
  value: string;
  checked?: boolean;
  width?: number;
  height?: number;
  readOnly?: boolean;
  style?: Style;
}

function parseRadioButton(element: P5Element): FormeNode {
  const props = decodeProps(element, 'RadioButton') as RadioButtonProps;

  return {
    kind: {
      type: 'RadioButton',
      name: props.name,
      value: props.value,
      checked: props.checked ?? false,
      width: props.width ?? 14,
      height: props.height ?? 14,
      read_only: props.readOnly ?? false,
    },
    style: mapStyle(props.style),
    children: [],
  };
}

// ─── Charts ──────────────────────────────────────────────────────────

/**
 * Parse one chart placeholder. The camelCase-to-snake_case prop
 * mapping (and its defaults) lives in the shared kind builders, the
 * same functions the react adapter serializes with, so the two
 * adapters cannot drift.
 */
function parseChart<P extends ChartPlaceholderProps>(
  element: P5Element,
  component: string,
  buildKind: (props: P) => FormeNodeKind
): FormeNode {
  const props = decodeProps(element, component) as P;

  return {
    kind: buildKind(props),
    style: mapStyle(props.style),
    children: [],
  };
}

// ─── Whitespace normalization ────────────────────────────────────────

/**
 * Normalize template text to JSX-equivalent semantics - the same
 * algorithm Babel, TypeScript, and esbuild apply to JSX text literals:
 *
 * - lines are trimmed (leading whitespace on all but the first line,
 *   trailing whitespace on all but the last line) and joined with a
 *   single space; whitespace-only lines are dropped
 * - tabs count as spaces
 * - only ASCII space/tab are trimmed, so non-breaking spaces survive
 */
function cleanJsxText(text: string): string {
  const lines = text.split(/\r\n|\n|\r/);
  let lastNonEmptyLine = 0;
  for (let i = 0; i < lines.length; i++) {
    if (/[^ \t]/.test(lines[i])) lastNonEmptyLine = i;
  }

  let str = '';
  for (let i = 0; i < lines.length; i++) {
    const isFirstLine = i === 0;
    const isLastLine = i === lines.length - 1;
    const isLastNonEmptyLine = i === lastNonEmptyLine;

    let trimmed = lines[i].replace(/\t/g, ' ');
    if (!isFirstLine) trimmed = trimmed.replace(/^ +/, '');
    if (!isLastLine) trimmed = trimmed.replace(/ +$/, '');

    if (trimmed) {
      if (!isLastNonEmptyLine) trimmed += ' ';
      str += trimmed;
    }
  }
  return str;
}

function findDocumentRoot(nodes: P5Node[]): P5Element {
  for (const node of nodes) {
    if (isElement(node) && node.tagName === 'forme-document') {
      return node;
    }
  }
  throw new Error('Top-level element must be <Document>');
}

/** Decode the JSON `props` attribute of a placeholder element. */
function decodeProps(element: P5Element, component: string): unknown {
  const attr = element.attrs.find(a => a.name === 'props');
  if (attr === undefined) return {};
  try {
    return JSON.parse(attr.value, reviveBytesMarker) as unknown;
  } catch (err) {
    throw new Error(
      `[Forme] <${component}>: failed to decode props attribute: ${err instanceof Error ? err.message : String(err)}`
    );
  }
}
