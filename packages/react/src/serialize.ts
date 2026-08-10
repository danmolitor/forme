import { type ReactElement, type ReactNode, isValidElement, Children, Fragment } from 'react';
import { Document, Page, View, Text, H1, H2, H3, H4, H5, H6, OrderedList, UnorderedList, ListItem, Strong, Em, Code, Link, Image, Table, Row, Cell, Fixed, Svg, QrCode, Barcode, Canvas, Watermark, PageBreak, BarChart, LineChart, PieChart, AreaChart, DotPlot, TextField, Checkbox, Dropdown, RadioButton } from './components.js';
import { Font } from './font.js';
import { mapStyle, parseColor, expandEdges, mapColumnWidth, mergeFonts, recordCanvasOperations, mapListMarker, HEADING_DEFAULTS, STRONG_DEFAULTS, EM_DEFAULTS, CODE_DEFAULTS, LINK_DEFAULTS, buildBarChartKind, buildLineChartKind, buildPieChartKind, buildAreaChartKind, buildDotPlotKind } from '@formepdf/shared';
import {
  isRefMarker, getRefPath,
  isEachMarker, getEachPath, getEachTemplate,
  isExprMarker, getExpr,
  REF_SENTINEL, REF_SENTINEL_END,
} from './template-proxy.js';
import type {
  Style,
  Edges,
  ColumnDef,
  TextRun,
  DocumentProps,
  FormeDocument,
  FormeNode,
  FormeNodeKind,
  FormePageConfig,
  FormePageSize,
  FormeEdges,
  FormeColumnDef,
  FormeListMarkerType,
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

// ─── Nesting validation ──────────────────────────────────────────────

type ParentContext = 'Document' | 'Page' | 'View' | 'Table' | 'Row' | 'Cell' | 'Fixed' | null;

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

// ─── Source location extraction ─────────────────────────────────────

function extractSourceLocation(element: ReactElement): { file: string; line: number; column: number } | undefined {
  // Check globalThis.__formeSourceMap (populated by CLI dev server's JSX shim for React 19+)
  const map = (globalThis as any).__formeSourceMap as WeakMap<object, { file: string; line: number; column: number }> | undefined;
  if (map) {
    const source = map.get(element);
    if (source) return source;
  }
  // Fallback to _source for React 18 and earlier
  const s = (element as any)._source;
  if (s && s.fileName) {
    return { file: s.fileName, line: s.lineNumber, column: s.columnNumber };
  }
  return undefined;
}

// ─── Version-independent type checks ─────────────────────────────────

/**
 * Check if a component type is `Document`, even across different package versions.
 * Uses `__formeType` marker first (survives minification), then falls back to
 * `displayName` / `name` for legacy builds without the marker.
 */
function isDocumentType(type: unknown): boolean {
  if (type === Document) return true;
  if (typeof type === 'function') {
    return (type as any).__formeType === 'Document'
      || (type as any).displayName === 'Document'
      || (type as any).name === 'Document';
  }
  return false;
}

// ─── Component resolution ────────────────────────────────────────────

/**
 * Resolve wrapper function components (e.g. `<MyReport data={...} />`) by
 * calling them until we reach a Forme primitive like `<Document>`. This lets
 * users pass custom components directly to `renderDocument()` /
 * `serialize()` without manually invoking them first.
 */
function resolveElement(element: ReactElement): ReactElement {
  let resolved = element;
  for (let i = 0; i < 10 && typeof resolved.type === 'function' && !isDocumentType(resolved.type); i++) {
    const result = callComponent(resolved.type as (props: unknown) => ReactElement, resolved.props);
    if (!isValidElement(result)) break;
    resolved = result;
  }
  return resolved;
}

/**
 * Call a function component directly (outside React's render cycle).
 * React Compiler injects `useMemoCache` hooks into compiled components,
 * which throws when called outside a render context. We catch that and
 * give a clear error message pointing to the `'use no memo'` directive.
 */
function callComponent(fn: (props: unknown) => unknown, props: unknown): unknown {
  try {
    return fn(props);
  } catch (err) {
    if (err instanceof Error && /hook|useMemoCache|Invalid hook call/i.test(err.message)) {
      const name = (fn as any).displayName || fn.name || 'Unknown';
      throw new Error(
        `Component "${name}" appears to be compiled by React Compiler, which injects hooks ` +
        `that cannot run outside of React's render cycle. Forme's serialize() calls components ` +
        `directly to walk the element tree.\n\n` +
        `Fix: Add 'use no memo' at the top of the component to opt it out of React Compiler:\n\n` +
        `  function ${name}(props) {\n` +
        `    'use no memo';\n` +
        `    return <Document>...</Document>;\n` +
        `  }\n\n` +
        `Alternatively, call the component yourself before passing to renderDocument():\n\n` +
        `  const element = <${name} data={data} />;\n` +
        `  // becomes:\n` +
        `  const element = ${name}({ data });\n` +
        `  await renderDocument(element);`
      );
    }
    throw err;
  }
}

// ─── Public API ──────────────────────────────────────────────────────

/**
 * Serialize a React element tree into a Forme JSON document object.
 * The top-level element must be a <Document> (or a component that returns one).
 */
export function serialize(element: ReactElement): FormeDocument {
  element = resolveElement(element);

  if (!isDocumentType(element.type)) {
    throw new Error('Top-level element must be <Document>');
  }

  const props = element.props as DocumentProps & { children?: unknown };
  const childElements = flattenChildren(props.children);

  // Separate Page children from content children
  const pageNodes: FormeNode[] = [];
  const contentNodes: FormeNode[] = [];

  for (const child of childElements) {
    if (isValidElement(child) && child.type === Page) {
      pageNodes.push(serializePage(child));
    } else {
      const node = serializeChild(child, 'Document');
      if (node) contentNodes.push(node);
    }
  }

  // If there are page nodes, use them. Otherwise wrap content in a default page.
  let children: FormeNode[];
  if (pageNodes.length > 0) {
    // Any loose content nodes get added to the last page's children
    if (contentNodes.length > 0) {
      const lastPage = pageNodes[pageNodes.length - 1];
      lastPage.children.push(...contentNodes);
    }
    children = pageNodes;
  } else if (contentNodes.length > 0) {
    children = contentNodes;
  } else {
    children = [];
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

// ─── Page serialization ──────────────────────────────────────────────

function serializePage(element: ReactElement): FormeNode {
  const props = element.props as {
    size?: string | { width: number; height: number };
    margin?: number | string | number[] | Edges;
    style?: Style;
    children?: unknown;
    backgroundImage?: string;
    backgroundOpacity?: number;
    backgroundSize?: 'fill' | 'cover' | 'contain';
    backgroundPosition?: 'center' | 'top-left' | 'top-right' | 'bottom-left' | 'bottom-right';
  };

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
  const childElements = flattenChildren(props.children);
  const children = serializeChildren(childElements, 'Page');

  return {
    kind: { type: 'Page', config },
    style: props.style ? mapStyle(props.style) : {},
    children,
    sourceLocation: extractSourceLocation(element),
  };
}

// ─── Node serialization ─────────────────────────────────────────────

function serializeChild(child: unknown, parent: ParentContext = null): FormeNode | null {
  if (child === null || child === undefined || typeof child === 'boolean') {
    return null;
  }

  if (typeof child === 'string') {
    return {
      kind: { type: 'Text', content: child },
      style: {},
      children: [],
    };
  }

  if (typeof child === 'number') {
    return {
      kind: { type: 'Text', content: String(child) },
      style: {},
      children: [],
    };
  }

  if (!isValidElement(child)) {
    // Detect HTML elements and give helpful suggestion
    if (typeof child === 'object' && child !== null && 'type' in child) {
      const t = (child as { type: unknown }).type;
      if (typeof t === 'string') {
        const suggestions: Record<string, string> = {
          div: 'View', span: 'Text', p: 'Text', h1: 'Text', h2: 'Text',
          h3: 'Text', img: 'Image', table: 'Table', tr: 'Row', td: 'Cell',
        };
        const suggestion = suggestions[t];
        if (suggestion) {
          throw new Error(
            `HTML element <${t}> is not supported. Use <${suggestion}> instead.`
          );
        }
      }
    }
    return null;
  }

  const element = child as ReactElement;

  if (element.type === View) {
    return serializeView(element, parent);
  }
  if (element.type === Text) {
    return serializeText(element);
  }
  if (element.type === H1) return serializeHeading(element, 1);
  if (element.type === H2) return serializeHeading(element, 2);
  if (element.type === H3) return serializeHeading(element, 3);
  if (element.type === H4) return serializeHeading(element, 4);
  if (element.type === H5) return serializeHeading(element, 5);
  if (element.type === H6) return serializeHeading(element, 6);
  if (element.type === OrderedList) return serializeList(element, true);
  if (element.type === UnorderedList) return serializeList(element, false);
  if (element.type === ListItem) return serializeListItem(element);
  if (element.type === Image) {
    return serializeImage(element);
  }
  if (element.type === Table) {
    return serializeTable(element, parent);
  }
  if (element.type === Row) {
    validateNesting('Row', parent);
    return serializeRow(element);
  }
  if (element.type === Cell) {
    validateNesting('Cell', parent);
    return serializeCell(element);
  }
  if (element.type === Fixed) {
    return serializeFixed(element);
  }
  if (element.type === Svg) {
    return serializeSvg(element);
  }
  if (element.type === QrCode) {
    return serializeQrCode(element);
  }
  if (element.type === Barcode) {
    return serializeBarcode(element);
  }
  if (element.type === TextField) {
    return serializeTextField(element);
  }
  if (element.type === Checkbox) {
    return serializeCheckbox(element);
  }
  if (element.type === Dropdown) {
    return serializeDropdown(element);
  }
  if (element.type === RadioButton) {
    return serializeRadioButton(element);
  }
  if (element.type === Canvas) {
    return serializeCanvas(element);
  }
  if (element.type === Watermark) {
    return serializeWatermark(element);
  }
  if (element.type === BarChart) {
    return serializeBarChart(element);
  }
  if (element.type === LineChart) {
    return serializeLineChart(element);
  }
  if (element.type === PieChart) {
    return serializePieChart(element);
  }
  if (element.type === AreaChart) {
    return serializeAreaChart(element);
  }
  if (element.type === DotPlot) {
    return serializeDotPlot(element);
  }
  if (element.type === PageBreak) {
    return {
      kind: { type: 'PageBreak' },
      style: {},
      children: [],
      sourceLocation: extractSourceLocation(element),
    };
  }
  if (element.type === Page) {
    validateNesting('Page', parent);
    return serializePage(element);
  }
  if (isDocumentType(element.type)) {
    // Nested Document — just serialize its children
    const props = element.props as { children?: unknown };
    const childElements = flattenChildren(props.children);
    const nodes = serializeChildren(childElements, parent);
    return nodes.length === 1 ? nodes[0] : {
      kind: { type: 'View' },
      style: {},
      children: nodes,
    };
  }

  // Unknown component — try to call it as a function component
  if (typeof element.type === 'function') {
    const result = callComponent(element.type as (props: unknown) => unknown, element.props);
    if (isValidElement(result)) {
      return serializeChild(result, parent);
    }
    return null;
  }

  return null;
}

function serializeView(element: ReactElement, _parent: ParentContext = null): FormeNode {
  const props = element.props as { style?: Style; wrap?: boolean; bookmark?: string; href?: string; children?: unknown };
  const style = mapStyle(props.style);
  if (props.wrap !== undefined) {
    style.wrap = props.wrap;
  }
  const childElements = flattenChildren(props.children);
  const children = serializeChildren(childElements, 'View');

  const node: FormeNode = {
    kind: { type: 'View' },
    style,
    children,
    sourceLocation: extractSourceLocation(element),
  };
  if (props.bookmark) node.bookmark = props.bookmark;
  if (props.href) node.href = props.href;

  return node;
}

/** Return the component-default style for a recognized inline component,
 *  or `null` if the element isn't a recognized inline component.
 *  `Text` returns `{}` because it carries no defaults beyond the user's style. */
function inlineDefaults(type: unknown): Style | null {
  switch (type) {
    case Text: return {};
    case Strong: return STRONG_DEFAULTS;
    case Em: return EM_DEFAULTS;
    case Code: return CODE_DEFAULTS;
    case Link: return LINK_DEFAULTS;
    default: return null;
  }
}

/**
 * Recursively flatten children of a `<Text>` into TextRuns, accumulating
 * styles and href as we descend. Outer style is overlaid by each inline
 * component's defaults, which are in turn overlaid by the user-supplied
 * `style` on that element — so user style wins at every level while
 * still inheriting whatever wasn't explicitly set.
 *
 * E.g. `<Strong><Em>both</Em></Strong>` produces a single run with
 * `{ fontWeight: 700, fontStyle: 'italic' }`. A `<Link href>` nested in a
 * `<Strong>` produces a bold underlined blue run with the href set.
 */
function buildTextRuns(
  children: unknown,
  accStyle: Style = {},
  accHref: string | undefined = undefined,
): TextRun[] {
  const out: TextRun[] = [];
  const elements = flattenChildren(children);
  for (const child of elements) {
    if (child === null || child === undefined || typeof child === 'boolean') continue;
    if (typeof child === 'string' || typeof child === 'number') {
      const content = String(child);
      if (content === '') continue;
      const run: TextRun = { content };
      if (Object.keys(accStyle).length > 0) run.style = mapStyle(accStyle);
      if (accHref) run.href = accHref;
      out.push(run);
      continue;
    }
    if (!isValidElement(child)) continue;
    const defaults = inlineDefaults(child.type);
    if (defaults === null) continue; // unknown element inside <Text> — skip
    const childProps = child.props as { style?: Style; href?: string; children?: unknown };
    // Cascade: outer accumulated → this component's defaults → user style.
    // Later spreads win, so user style overrides defaults and outer.
    const nextStyle: Style = { ...accStyle, ...defaults, ...(childProps.style || {}) };
    const nextHref = childProps.href ?? accHref;
    out.push(...buildTextRuns(childProps.children, nextStyle, nextHref));
  }
  return out;
}

function serializeText(element: ReactElement): FormeNode {
  const props = element.props as { style?: Style; href?: string; bookmark?: string; children?: unknown };
  const childElements = flattenChildren(props.children);

  // Detect mixed content: any inline element child (<Text>, <Strong>, <Em>,
  // <Code>, <Link>) means we need TextRuns rather than a flat content string.
  const hasInlineChild = childElements.some(
    c => isValidElement(c) && inlineDefaults(c.type) !== null
  );

  const kind: FormeNodeKind & { type: 'Text' } = { type: 'Text', content: '' };

  if (hasInlineChild) {
    kind.runs = buildTextRuns(props.children);
  } else {
    kind.content = flattenTextContent(props.children);
  }

  if (props.href) kind.href = props.href;

  const node: FormeNode = {
    kind,
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
  if (props.bookmark) node.bookmark = props.bookmark;

  return node;
}

/**
 * Serialize an H1-H6 element. Mirrors serializeText for the children
 * machinery (mixed strings + inline-formatting components produce runs)
 * but emits the engine's `Heading { level }` node kind so the tagged-PDF
 * builder can pick up the semantic role. Default styles per level are
 * merged BEFORE the user's `style` prop, so user values win.
 */
function serializeHeading(
  element: ReactElement,
  level: 1 | 2 | 3 | 4 | 5 | 6,
): FormeNode {
  const props = element.props as {
    style?: Style;
    href?: string;
    bookmark?: string;
    children?: unknown;
  };
  const childElements = flattenChildren(props.children);
  const hasInlineChild = childElements.some(
    (c) => isValidElement(c) && inlineDefaults(c.type) !== null,
  );

  const kind: FormeNodeKind & { type: 'Heading' } = {
    type: 'Heading',
    level,
    content: '',
  };
  if (hasInlineChild) {
    kind.runs = buildTextRuns(props.children);
  } else {
    kind.content = flattenTextContent(props.children);
  }
  if (props.href) kind.href = props.href;

  // Defaults underlay; user style wins on conflicting keys.
  const mergedStyle: Style = { ...HEADING_DEFAULTS[level], ...(props.style || {}) };

  const node: FormeNode = {
    kind,
    style: mapStyle(mergedStyle),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
  if (props.bookmark) node.bookmark = props.bookmark;

  return node;
}

function serializeList(element: ReactElement, ordered: boolean): FormeNode {
  const props = element.props as {
    type?: string;
    marker?: string;
    start?: number;
    style?: Style;
    bookmark?: string;
    children?: unknown;
  };

  const markerType: FormeListMarkerType = ordered
    ? mapListMarker(props.type, 'decimal')
    : mapListMarker(props.marker, 'disc');

  const start = typeof props.start === 'number' && props.start >= 1 ? props.start : 1;

  // Children must be ListItems — anything else is silently dropped to
  // keep the serializer tolerant. (We don't throw on stray content because
  // a Fragment / null child in JSX is too common to be a real error.)
  const childElements = flattenChildren(props.children).filter(
    (c) => isValidElement(c) && c.type === ListItem,
  ) as ReactElement[];
  const children = childElements.map((c) => serializeListItem(c));

  const node: FormeNode = {
    kind: { type: 'List', ordered, marker_type: markerType, start },
    style: mapStyle(props.style),
    children,
    sourceLocation: extractSourceLocation(element),
  };
  if (props.bookmark) node.bookmark = props.bookmark;
  return node;
}

function serializeListItem(element: ReactElement): FormeNode {
  const props = element.props as { style?: Style; children?: unknown };
  // ListItem content is layed out by the engine using layout_children, so
  // we serialize whatever the user put inside as the node's children. This
  // covers plain strings, JSX text, nested lists, formatted runs via
  // <Text>, etc.
  const rawChildren = flattenChildren(props.children);
  const children: FormeNode[] = [];
  for (const c of rawChildren) {
    if (typeof c === 'string' || typeof c === 'number') {
      // Auto-wrap raw strings in a Text node so the engine has something
      // concrete to render. Matches the convention everywhere else.
      children.push({
        kind: { type: 'Text', content: String(c) },
        style: {},
        children: [],
      });
    } else if (isValidElement(c)) {
      const node = serializeChild(c, null);
      if (node) children.push(node);
    }
  }
  return {
    kind: { type: 'ListItem' },
    style: mapStyle(props.style),
    children,
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeImage(element: ReactElement): FormeNode {
  const props = element.props as { src: string; width?: number; height?: number; style?: Style; href?: string; alt?: string };
  const kind: FormeNodeKind = { type: 'Image', src: props.src };
  if (props.width !== undefined) (kind as { width?: number }).width = props.width;
  if (props.height !== undefined) (kind as { height?: number }).height = props.height;

  const node: FormeNode = {
    kind,
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
  if (props.href) node.href = props.href;
  if (props.alt) node.alt = props.alt;
  return node;
}

function serializeTable(element: ReactElement, _parent: ParentContext = null): FormeNode {
  const props = element.props as { columns?: ColumnDef[]; style?: Style; children?: unknown };
  const columns: FormeColumnDef[] = (props.columns ?? []).map(col => ({
    width: mapColumnWidth(col.width),
  }));

  const childElements = flattenChildren(props.children);
  const children = serializeChildren(childElements, 'Table');

  return {
    kind: { type: 'Table', columns },
    style: mapStyle(props.style),
    children,
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeRow(element: ReactElement): FormeNode {
  const props = element.props as { header?: boolean; style?: Style; children?: unknown };
  const childElements = flattenChildren(props.children);
  const children = serializeChildren(childElements, 'Row');

  return {
    kind: { type: 'TableRow', is_header: props.header ?? false },
    style: mapStyle(props.style),
    children,
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeCell(element: ReactElement): FormeNode {
  const props = element.props as { colSpan?: number; rowSpan?: number; style?: Style; children?: unknown };
  const childElements = flattenChildren(props.children);
  const children = serializeChildren(childElements, 'Cell');

  return {
    kind: { type: 'TableCell', col_span: props.colSpan ?? 1, row_span: props.rowSpan ?? 1 },
    style: mapStyle(props.style),
    children,
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeFixed(element: ReactElement): FormeNode {
  const props = element.props as { position: 'header' | 'footer'; style?: Style; bookmark?: string; children?: unknown };
  const position = props.position === 'header' ? 'Header' as const : 'Footer' as const;
  const childElements = flattenChildren(props.children);
  const children = serializeChildren(childElements, 'Fixed');

  const node: FormeNode = {
    kind: { type: 'Fixed', position },
    style: mapStyle(props.style),
    children,
    sourceLocation: extractSourceLocation(element),
  };
  if (props.bookmark) node.bookmark = props.bookmark;

  return node;
}

function serializeSvg(element: ReactElement): FormeNode {
  const props = element.props as { width: number; height: number; viewBox?: string; content?: string; style?: Style; href?: string; alt?: string; children?: ReactNode };
  const content = props.content ?? (props.children ? svgChildrenToString(props.children) : '');
  const kind: FormeNodeKind = {
    type: 'Svg',
    width: props.width,
    height: props.height,
    content,
  };
  if (props.viewBox) (kind as { view_box?: string }).view_box = props.viewBox;

  const node: FormeNode = {
    kind,
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
  if (props.href) node.href = props.href;
  if (props.alt) node.alt = props.alt;
  return node;
}

/** Map camelCase SVG prop names to kebab-case attribute names. */
const svgCamelToKebab: Record<string, string> = {
  strokeWidth: 'stroke-width',
  strokeLinecap: 'stroke-linecap',
  strokeLinejoin: 'stroke-linejoin',
  strokeMiterlimit: 'stroke-miterlimit',
  strokeDasharray: 'stroke-dasharray',
  strokeDashoffset: 'stroke-dashoffset',
  strokeOpacity: 'stroke-opacity',
  fillOpacity: 'fill-opacity',
  fillRule: 'fill-rule',
  clipPath: 'clip-path',
  clipRule: 'clip-rule',
};

function escapeXmlAttr(s: string): string {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

function svgChildrenToString(children: ReactNode): string {
  let result = '';
  Children.forEach(children, (child) => {
    if (typeof child === 'string') {
      result += child.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
      return;
    }
    if (!isValidElement(child)) return;
    const tag = typeof child.type === 'string' ? child.type : null;
    if (!tag) return;
    const { children: nested, ...attrs } = child.props as Record<string, unknown>;
    const attrStr = Object.entries(attrs)
      .filter(([, v]) => v !== undefined && v !== null)
      .map(([k, v]) => {
        const name = svgCamelToKebab[k] ?? k;
        return `${name}="${escapeXmlAttr(String(v))}"`;
      })
      .join(' ');
    const open = attrStr ? `<${tag} ${attrStr}` : `<${tag}`;
    if (nested) {
      result += `${open}>${svgChildrenToString(nested as ReactNode)}</${tag}>`;
    } else {
      result += `${open}/>`;
    }
  });
  return result;
}

function serializeQrCode(element: ReactElement): FormeNode {
  const props = element.props as QrCodeProps;
  const kind: FormeNodeKind = { type: 'QrCode', data: props.data } as FormeNodeKind;
  if (props.size !== undefined) (kind as Record<string, unknown>).size = props.size;
  const style = mapStyle(props.style);
  if (props.color) style.color = parseColor(props.color);
  return {
    kind,
    style,
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeBarcode(element: ReactElement): FormeNode {
  const props = element.props as BarcodeProps;
  const kind: FormeNodeKind = {
    type: 'Barcode',
    data: props.data,
    format: props.format ?? 'Code128',
    height: props.height ?? 60,
  } as FormeNodeKind;
  if (props.width !== undefined) (kind as Record<string, unknown>).width = props.width;
  const style = mapStyle(props.style);
  if (props.color) style.color = parseColor(props.color);
  return {
    kind,
    style,
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeTextField(element: ReactElement): FormeNode {
  const props = element.props as TextFieldProps;
  const kind: FormeNodeKind = {
    type: 'TextField',
    name: props.name,
    width: props.width,
    height: props.height ?? 24,
    multiline: props.multiline ?? false,
    password: props.password ?? false,
    read_only: props.readOnly ?? false,
    font_size: props.fontSize ?? 12,
  } as FormeNodeKind;
  if (props.value !== undefined) (kind as Record<string, unknown>).value = props.value;
  if (props.placeholder !== undefined) (kind as Record<string, unknown>).placeholder = props.placeholder;
  if (props.maxLength !== undefined) (kind as Record<string, unknown>).max_length = props.maxLength;
  return {
    kind,
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeCheckbox(element: ReactElement): FormeNode {
  const props = element.props as CheckboxProps;
  const kind: FormeNodeKind = {
    type: 'Checkbox',
    name: props.name,
    checked: props.checked ?? false,
    width: props.width ?? 14,
    height: props.height ?? 14,
    read_only: props.readOnly ?? false,
  } as FormeNodeKind;
  return {
    kind,
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeDropdown(element: ReactElement): FormeNode {
  const props = element.props as DropdownProps;
  const kind: FormeNodeKind = {
    type: 'Dropdown',
    name: props.name,
    options: props.options,
    width: props.width,
    height: props.height ?? 24,
    read_only: props.readOnly ?? false,
    font_size: props.fontSize ?? 12,
  } as FormeNodeKind;
  if (props.value !== undefined) (kind as Record<string, unknown>).value = props.value;
  return {
    kind,
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeRadioButton(element: ReactElement): FormeNode {
  const props = element.props as RadioButtonProps;
  const kind: FormeNodeKind = {
    type: 'RadioButton',
    name: props.name,
    value: props.value,
    checked: props.checked ?? false,
    width: props.width ?? 14,
    height: props.height ?? 14,
    read_only: props.readOnly ?? false,
  } as FormeNodeKind;
  return {
    kind,
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeCanvas(element: ReactElement): FormeNode {
  const props = element.props as CanvasProps;
  const operations = recordCanvasOperations(props.draw);

  return {
    kind: { type: 'Canvas', width: props.width, height: props.height, operations },
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeWatermark(element: ReactElement): FormeNode {
  const props = element.props as WatermarkProps;
  const fontSize = props.fontSize ?? 60;
  const angle = props.angle ?? -45;
  const colorStr = props.color ?? 'rgba(0,0,0,0.1)';

  // Parse color — extract alpha for opacity
  const parsedColor = parseColor(colorStr);
  const style = mapStyle(props.style);
  style.color = { r: parsedColor.r, g: parsedColor.g, b: parsedColor.b, a: 1 };
  // Multiply color alpha with any existing opacity
  const colorOpacity = parsedColor.a;
  const styleOpacity = (style.opacity !== undefined) ? style.opacity as number : 1;
  style.opacity = colorOpacity * styleOpacity;
  style.fontSize = fontSize;

  return {
    kind: { type: 'Watermark', text: props.text, font_size: fontSize, angle },
    style,
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeBarChart(element: ReactElement): FormeNode {
  const props = element.props as BarChartProps;
  return {
    kind: buildBarChartKind(props),
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeLineChart(element: ReactElement): FormeNode {
  const props = element.props as LineChartProps;
  return {
    kind: buildLineChartKind(props),
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

function serializePieChart(element: ReactElement): FormeNode {
  const props = element.props as PieChartProps;
  return {
    kind: buildPieChartKind(props),
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeAreaChart(element: ReactElement): FormeNode {
  const props = element.props as AreaChartProps;
  return {
    kind: buildAreaChartKind(props),
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

function serializeDotPlot(element: ReactElement): FormeNode {
  const props = element.props as DotPlotProps;
  return {
    kind: buildDotPlotKind(props),
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
}

// ─── Children helpers ────────────────────────────────────────────────

function flattenChildren(children: unknown): unknown[] {
  const result: unknown[] = [];
  Children.forEach(children as React.ReactNode, child => {
    if (Array.isArray(child)) {
      result.push(...child.flatMap(c => flattenChildren(c)));
    } else if (isValidElement(child) && child.type === Fragment) {
      const fragProps = child.props as { children?: unknown };
      result.push(...flattenChildren(fragProps.children));
    } else {
      result.push(child);
    }
  });
  return result;
}

function serializeChildren(children: unknown[], parent: ParentContext = null): FormeNode[] {
  const nodes: FormeNode[] = [];
  for (const child of children) {
    const node = serializeChild(child, parent);
    if (node) nodes.push(node);
  }
  return nodes;
}

/**
 * Flatten all text content within a <Text> element to a single string.
 * Nested <Text> children have their content extracted and concatenated.
 */
function flattenTextContent(children: unknown): string {
  if (children === null || children === undefined) return '';
  if (typeof children === 'string') return children;
  if (typeof children === 'number') return String(children);
  if (typeof children === 'boolean') return '';

  if (Array.isArray(children)) {
    return children.map(c => flattenTextContent(c)).join('');
  }

  if (isValidElement(children)) {
    const element = children as ReactElement;
    if (element.type === Text) {
      const props = element.props as { children?: unknown };
      return flattenTextContent(props.children);
    }
    // For other elements inside Text, try to extract text content
    const props = element.props as { children?: unknown };
    return flattenTextContent(props.children);
  }

  // React.Children.toArray for iterables
  const arr: unknown[] = [];
  Children.forEach(children as React.ReactNode, c => arr.push(c));
  if (arr.length > 0) {
    return arr.map(c => flattenTextContent(c)).join('');
  }

  return String(children);
}

// ─── Template serialization ─────────────────────────────────────────
//
// Parallel to `serialize()` but detects proxy markers and expr markers,
// converting them to `$ref`, `$each`, `$if`, and operator nodes.

/**
 * Serialize a React element tree into a Forme template JSON document.
 * Like `serialize()` but with expression marker detection for template compilation.
 */
export function serializeTemplate(element: ReactElement): Record<string, unknown> {
  element = resolveElement(element);

  if (!isDocumentType(element.type)) {
    throw new Error('Top-level element must be <Document>');
  }

  const props = element.props as { title?: string; author?: string; subject?: string; creator?: string; children?: unknown } & DocumentProps;
  const childElements = flattenTemplateChildren(props.children);

  const pageNodes: unknown[] = [];
  const contentNodes: unknown[] = [];

  for (const child of childElements) {
    if (isValidElement(child) && child.type === Page) {
      pageNodes.push(serializeTemplatePage(child));
    } else {
      const node = serializeTemplateChild(child, 'Document');
      if (node !== null) contentNodes.push(node);
    }
  }

  let children: unknown[];
  if (pageNodes.length > 0) {
    if (contentNodes.length > 0) {
      const lastPage = pageNodes[pageNodes.length - 1] as { children: unknown[] };
      lastPage.children.push(...contentNodes);
    }
    children = pageNodes;
  } else if (contentNodes.length > 0) {
    children = contentNodes;
  } else {
    children = [];
  }

  const metadata: Record<string, unknown> = {};
  if (props.title !== undefined) metadata.title = processTemplateValue(props.title);
  if (props.author !== undefined) metadata.author = processTemplateValue(props.author);
  if (props.subject !== undefined) metadata.subject = processTemplateValue(props.subject);
  if (props.creator !== undefined) metadata.creator = processTemplateValue(props.creator);
  if (props.lang !== undefined) metadata.lang = processTemplateValue(props.lang);

  const mergedFonts = mergeFonts(Font.getRegistered(), props.fonts);

  const result: Record<string, unknown> = {
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

function serializeTemplatePage(element: ReactElement): Record<string, unknown> {
  const props = element.props as { size?: string | { width: number; height: number }; margin?: number | string | number[] | Edges; children?: unknown };

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
  const childElements = flattenTemplateChildren(props.children);
  const children = serializeTemplateChildren(childElements, 'Page');

  return {
    kind: { type: 'Page', config },
    style: {},
    children,
  };
}

function serializeTemplateChild(child: unknown, parent: ParentContext = null): unknown | null {
  if (child === null || child === undefined || typeof child === 'boolean') {
    return null;
  }

  // Check for each marker (from .map() on proxy)
  if (isEachMarker(child)) {
    const path = getEachPath(child);
    const template = getEachTemplate(child);
    // The template is the JSX element returned from the .map() callback
    const serializedTemplate = isValidElement(template as ReactElement)
      ? serializeTemplateChild(template, parent)
      : processTemplateValue(template);
    return {
      $each: { $ref: path },
      as: '$item',
      template: serializedTemplate,
    };
  }

  // Check for expr marker
  if (isExprMarker(child)) {
    return serializeExprValues(getExpr(child), parent);
  }

  // Check for ref sentinel strings
  if (typeof child === 'string') {
    const processed = processTemplateString(child);
    if (processed !== null) return processed;
    return {
      kind: { type: 'Text', content: child },
      style: {},
      children: [],
    };
  }

  if (typeof child === 'number') {
    return {
      kind: { type: 'Text', content: String(child) },
      style: {},
      children: [],
    };
  }

  if (!isValidElement(child)) return null;

  const element = child as ReactElement;

  if (element.type === View) return serializeTemplateView(element, parent);
  if (element.type === Text) return serializeTemplateText(element);
  if (element.type === Image) return serializeTemplateImage(element);
  if (element.type === Table) return serializeTemplateTable(element, parent);
  if (element.type === Row) {
    validateNesting('Row', parent);
    return serializeTemplateRow(element);
  }
  if (element.type === Cell) {
    validateNesting('Cell', parent);
    return serializeTemplateCell(element);
  }
  if (element.type === Fixed) return serializeTemplateFixed(element);
  if (element.type === Svg) return serializeSvg(element);
  if (element.type === QrCode) return serializeQrCode(element);
  if (element.type === Barcode) return serializeBarcode(element);
  if (element.type === TextField) return serializeTextField(element);
  if (element.type === Checkbox) return serializeCheckbox(element);
  if (element.type === Dropdown) return serializeDropdown(element);
  if (element.type === RadioButton) return serializeRadioButton(element);
  if (element.type === Canvas) return serializeCanvas(element);
  if (element.type === Watermark) return serializeWatermark(element);
  if (element.type === BarChart) return serializeBarChart(element);
  if (element.type === LineChart) return serializeLineChart(element);
  if (element.type === PieChart) return serializePieChart(element);
  if (element.type === AreaChart) return serializeAreaChart(element);
  if (element.type === DotPlot) return serializeDotPlot(element);
  if (element.type === PageBreak) {
    return { kind: { type: 'PageBreak' }, style: {}, children: [] };
  }
  if (element.type === Page) {
    validateNesting('Page', parent);
    return serializeTemplatePage(element);
  }

  // Unknown function component — call it
  if (typeof element.type === 'function') {
    const result = callComponent(element.type as (props: unknown) => unknown, element.props);
    if (isValidElement(result)) {
      return serializeTemplateChild(result, parent);
    }
    return null;
  }

  return null;
}

function serializeTemplateView(element: ReactElement, _parent: ParentContext = null): Record<string, unknown> {
  const props = element.props as { style?: Style; wrap?: boolean; bookmark?: string; href?: string; children?: unknown };
  const style = mapTemplateStyle(props.style);
  if (props.wrap !== undefined) style.wrap = props.wrap;
  const childElements = flattenTemplateChildren(props.children);
  const children = serializeTemplateChildren(childElements, 'View');

  const node: Record<string, unknown> = { kind: { type: 'View' }, style, children };
  if (props.bookmark) node.bookmark = props.bookmark;
  if (props.href) node.href = props.href;
  return node;
}

function serializeTemplateText(element: ReactElement): Record<string, unknown> {
  const props = element.props as { style?: Style; href?: string; bookmark?: string; children?: unknown };
  const childElements = flattenTemplateChildren(props.children);

  const hasTextChild = childElements.some(
    c => isValidElement(c) && c.type === Text
  );

  const kind: Record<string, unknown> = { type: 'Text', content: '' };

  if (hasTextChild) {
    const runs: Record<string, unknown>[] = [];
    for (const child of childElements) {
      if (typeof child === 'string' || typeof child === 'number') {
        const processed = typeof child === 'string' ? processTemplateString(child) : null;
        if (processed !== null) {
          runs.push({ content: processed });
        } else {
          runs.push({ content: String(child) });
        }
      } else if (isValidElement(child) && child.type === Text) {
        const childProps = child.props as { style?: Style; href?: string; children?: unknown };
        const run: Record<string, unknown> = {
          content: flattenTemplateTextContent(childProps.children),
        };
        if (childProps.style) run.style = mapTemplateStyle(childProps.style);
        if (childProps.href) run.href = childProps.href;
        runs.push(run);
      }
    }
    kind.runs = runs;
  } else {
    kind.content = flattenTemplateTextContent(props.children);
  }

  if (props.href) kind.href = props.href;

  const node: Record<string, unknown> = {
    kind,
    style: mapTemplateStyle(props.style),
    children: [],
  };
  if (props.bookmark) node.bookmark = props.bookmark;
  return node;
}

function serializeTemplateImage(element: ReactElement): Record<string, unknown> {
  const props = element.props as { src: string | unknown; width?: number; height?: number; style?: Style };
  const kind: Record<string, unknown> = { type: 'Image', src: processTemplateValue(props.src) };
  if (props.width !== undefined) kind.width = processTemplateValue(props.width);
  if (props.height !== undefined) kind.height = processTemplateValue(props.height);
  return { kind, style: mapTemplateStyle(props.style), children: [] };
}

function serializeTemplateTable(element: ReactElement, _parent: ParentContext = null): Record<string, unknown> {
  const props = element.props as { columns?: ColumnDef[]; style?: Style; children?: unknown };
  const columns: FormeColumnDef[] = (props.columns ?? []).map(col => ({
    width: mapColumnWidth(col.width),
  }));
  const childElements = flattenTemplateChildren(props.children);
  const children = serializeTemplateChildren(childElements, 'Table');
  return { kind: { type: 'Table', columns }, style: mapTemplateStyle(props.style), children };
}

function serializeTemplateRow(element: ReactElement): Record<string, unknown> {
  const props = element.props as { header?: boolean; style?: Style; children?: unknown };
  const childElements = flattenTemplateChildren(props.children);
  const children = serializeTemplateChildren(childElements, 'Row');
  return { kind: { type: 'TableRow', is_header: props.header ?? false }, style: mapTemplateStyle(props.style), children };
}

function serializeTemplateCell(element: ReactElement): Record<string, unknown> {
  const props = element.props as { colSpan?: number; rowSpan?: number; style?: Style; children?: unknown };
  const childElements = flattenTemplateChildren(props.children);
  const children = serializeTemplateChildren(childElements, 'Cell');
  return { kind: { type: 'TableCell', col_span: props.colSpan ?? 1, row_span: props.rowSpan ?? 1 }, style: mapTemplateStyle(props.style), children };
}

function serializeTemplateFixed(element: ReactElement): Record<string, unknown> {
  const props = element.props as { position: 'header' | 'footer'; style?: Style; bookmark?: string; children?: unknown };
  const position = props.position === 'header' ? 'Header' as const : 'Footer' as const;
  const childElements = flattenTemplateChildren(props.children);
  const children = serializeTemplateChildren(childElements, 'Fixed');
  const node: Record<string, unknown> = { kind: { type: 'Fixed', position }, style: mapTemplateStyle(props.style), children };
  if (props.bookmark) node.bookmark = props.bookmark;
  return node;
}

function serializeTemplateChildren(children: unknown[], parent: ParentContext = null): unknown[] {
  const nodes: unknown[] = [];
  for (const child of children) {
    const node = serializeTemplateChild(child, parent);
    if (node !== null) nodes.push(node);
  }
  return nodes;
}

/**
 * Flatten children without using React.Children.forEach, which rejects
 * proxy objects and markers. Handles arrays, Fragments, and raw values.
 */
function flattenTemplateChildren(children: unknown): unknown[] {
  if (children === null || children === undefined) return [];

  const result: unknown[] = [];

  if (Array.isArray(children)) {
    for (const child of children) {
      result.push(...flattenTemplateChildren(child));
    }
    return result;
  }

  // Fragment unwrapping
  if (isValidElement(children) && children.type === Fragment) {
    const fragProps = children.props as { children?: unknown };
    return flattenTemplateChildren(fragProps.children);
  }

  result.push(children);
  return result;
}

// ─── Expression value serialization ─────────────────────────────────

/**
 * Recursively process an expression object, serializing any React elements
 * found in its values (e.g. $if then/else branches).
 */
function serializeExprValues(expr: Record<string, unknown>, parent: ParentContext): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, val] of Object.entries(expr)) {
    if (isValidElement(val as ReactElement)) {
      result[key] = serializeTemplateChild(val, parent);
    } else if (Array.isArray(val)) {
      result[key] = val.map(v =>
        isValidElement(v as ReactElement) ? serializeTemplateChild(v, parent) : v
      );
    } else {
      result[key] = val;
    }
  }
  return result;
}

// ─── Template value processing ──────────────────────────────────────

/**
 * Process a value that may contain ref markers, expr markers, or proxy objects.
 * Returns the expression form or the original value.
 */
function processTemplateValue(v: unknown): unknown {
  if (typeof v === 'string') {
    if (isRefMarker(v)) {
      return { $ref: getRefPath(v) };
    }
    // Check for embedded sentinels in longer strings
    if (v.includes(REF_SENTINEL)) {
      return processTemplateInterpolatedString(v);
    }
    return v;
  }
  if (isExprMarker(v)) {
    return getExpr(v);
  }
  if (isEachMarker(v)) {
    return {
      $each: { $ref: getEachPath(v) },
      as: '$item',
      template: processTemplateValue(getEachTemplate(v)),
    };
  }
  // Proxy objects with toPrimitive
  if (typeof v === 'object' && v !== null && Symbol.toPrimitive in (v as object)) {
    const str = String(v);
    if (isRefMarker(str)) {
      return { $ref: getRefPath(str) };
    }
  }
  return v;
}

/**
 * Process a string that contains interpolated ref sentinels.
 * e.g. "Hello \0FORME_REF:name\0!" → {$concat: ["Hello ", {$ref: "name"}, "!"]}
 */
function processTemplateInterpolatedString(s: string): unknown {
  const parts: unknown[] = [];
  let remaining = s;

  while (remaining.length > 0) {
    const startIdx = remaining.indexOf(REF_SENTINEL);
    if (startIdx === -1) {
      parts.push(remaining);
      break;
    }

    if (startIdx > 0) {
      parts.push(remaining.slice(0, startIdx));
    }

    const afterSentinel = remaining.slice(startIdx + REF_SENTINEL.length);
    const endIdx = afterSentinel.indexOf(REF_SENTINEL_END);
    if (endIdx === -1) {
      parts.push(remaining);
      break;
    }

    const path = afterSentinel.slice(0, endIdx);
    parts.push({ $ref: path });
    remaining = afterSentinel.slice(endIdx + REF_SENTINEL_END.length);
  }

  if (parts.length === 1) return parts[0];
  return { $concat: parts };
}

/**
 * Process a string that might be a pure ref sentinel.
 * Returns the $ref node if it's a pure ref, null otherwise.
 */
function processTemplateString(s: string): unknown | null {
  if (isRefMarker(s)) {
    return { $ref: getRefPath(s) };
  }
  if (s.includes(REF_SENTINEL)) {
    return processTemplateInterpolatedString(s);
  }
  return null;
}

/**
 * Flatten text content within a <Text> element, detecting ref markers.
 * Returns either a plain string or a $ref/$concat expression.
 */
function flattenTemplateTextContent(children: unknown): unknown {
  if (children === null || children === undefined) return '';
  if (typeof children === 'boolean') return '';

  if (typeof children === 'string') {
    if (isRefMarker(children)) return { $ref: getRefPath(children) };
    if (children.includes(REF_SENTINEL)) return processTemplateInterpolatedString(children);
    return children;
  }

  if (typeof children === 'number') return String(children);

  if (isExprMarker(children)) return getExpr(children);

  // Proxy with toPrimitive
  if (typeof children === 'object' && children !== null && Symbol.toPrimitive in (children as object)) {
    const str = String(children);
    if (isRefMarker(str)) return { $ref: getRefPath(str) };
    return str;
  }

  if (Array.isArray(children)) {
    const parts = children.map(c => flattenTemplateTextContent(c));
    // If all parts are strings, join them
    if (parts.every(p => typeof p === 'string')) {
      return (parts as string[]).join('');
    }
    // Otherwise produce a $concat
    return { $concat: parts };
  }

  if (isValidElement(children)) {
    const element = children as ReactElement;
    if (element.type === Text) {
      const props = element.props as { children?: unknown };
      return flattenTemplateTextContent(props.children);
    }
    const props = element.props as { children?: unknown };
    return flattenTemplateTextContent(props.children);
  }

  const arr: unknown[] = [];
  Children.forEach(children as React.ReactNode, c => arr.push(c));
  if (arr.length > 0) {
    return flattenTemplateTextContent(arr);
  }

  return String(children);
}

/**
 * Map style, processing values that may contain template expressions.
 */
function mapTemplateStyle(style?: Style): Record<string, unknown> {
  if (!style) return {};
  // Use the regular mapStyle but then post-process values that contain markers
  const result = mapStyle(style) as Record<string, unknown>;
  return processTemplateStyleValues(result);
}

function processTemplateStyleValues(obj: Record<string, unknown>): Record<string, unknown> {
  const result: Record<string, unknown> = {};
  for (const [key, val] of Object.entries(obj)) {
    if (typeof val === 'object' && val !== null && !Array.isArray(val)) {
      result[key] = processTemplateStyleValues(val as Record<string, unknown>);
    } else {
      result[key] = processTemplateValue(val);
    }
  }
  return result;
}
