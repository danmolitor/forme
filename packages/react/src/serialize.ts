import { type ReactElement, isValidElement, Children, Fragment } from 'react';
import { Document, Page, View, Text, Image, Table, Row, Cell, Fixed, Svg, PageBreak } from './components.js';
import { Font, type FontRegistration } from './font.js';
import {
  isRefMarker, getRefPath,
  isEachMarker, getEachPath, getEachTemplate,
  isExprMarker, getExpr,
  REF_SENTINEL, REF_SENTINEL_END,
} from './template-proxy.js';
import type {
  Style,
  Edges,
  Corners,
  EdgeColors,
  ColumnDef,
  TextRun,
  DocumentProps,
  FormeDocument,
  FormeFont,
  FormeNode,
  FormeNodeKind,
  FormeStyle,
  FormePageConfig,
  FormePageSize,
  FormeEdges,
  FormeColumnDef,
  FormeColumnWidth,
  FormeDimension,
  FormeColor,
  FormeEdgeValues,
  FormeCornerValues,
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

// ─── Public API ──────────────────────────────────────────────────────

/**
 * Serialize a React element tree into a Forme JSON document object.
 * The top-level element must be a <Document>.
 */
export function serialize(element: ReactElement): FormeDocument {
  if (element.type !== Document) {
    throw new Error('Top-level element must be <Document>');
  }

  const props = element.props as { title?: string; author?: string; subject?: string; creator?: string; children?: unknown };
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

  // Merge global + document fonts (document fonts override on conflict)
  const mergedFonts = mergeFonts(Font.getRegistered(), (props as DocumentProps).fonts);

  const result: FormeDocument = {
    children,
    metadata,
    defaultPage: {
      size: 'A4',
      margin: { top: 54, right: 54, bottom: 54, left: 54 },
      wrap: true,
    },
  };

  if (mergedFonts.length > 0) {
    result.fonts = mergedFonts;
  }

  return result;
}

// ─── Page serialization ──────────────────────────────────────────────

function serializePage(element: ReactElement): FormeNode {
  const props = element.props as { size?: string | { width: number; height: number }; margin?: number | Edges; children?: unknown };

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
  const childElements = flattenChildren(props.children);
  const children = serializeChildren(childElements, 'Page');

  return {
    kind: { type: 'Page', config },
    style: {},
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
  if (element.type === Document) {
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
    const result = (element.type as (props: Record<string, unknown>) => unknown)(element.props as Record<string, unknown>);
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

function serializeText(element: ReactElement): FormeNode {
  const props = element.props as { style?: Style; href?: string; bookmark?: string; children?: unknown };
  const childElements = flattenChildren(props.children);

  // Check if any child is a <Text> element (inline runs)
  const hasTextChild = childElements.some(
    c => isValidElement(c) && c.type === Text
  );

  const kind: FormeNodeKind & { type: 'Text' } = { type: 'Text', content: '' };

  if (hasTextChild) {
    // Build runs from children
    const runs: TextRun[] = [];
    for (const child of childElements) {
      if (typeof child === 'string' || typeof child === 'number') {
        runs.push({ content: String(child) });
      } else if (isValidElement(child) && child.type === Text) {
        const childProps = child.props as { style?: Style; href?: string; children?: unknown };
        const run: TextRun = {
          content: flattenTextContent(childProps.children),
        };
        if (childProps.style) run.style = mapStyle(childProps.style);
        if (childProps.href) run.href = childProps.href;
        runs.push(run);
      }
    }
    kind.runs = runs;
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

function serializeImage(element: ReactElement): FormeNode {
  const props = element.props as { src: string; width?: number; height?: number; style?: Style };
  const kind: FormeNodeKind = { type: 'Image', src: props.src };
  if (props.width !== undefined) (kind as { width?: number }).width = props.width;
  if (props.height !== undefined) (kind as { height?: number }).height = props.height;

  return {
    kind,
    style: mapStyle(props.style),
    children: [],
    sourceLocation: extractSourceLocation(element),
  };
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
  const props = element.props as { width: number; height: number; viewBox?: string; content: string; style?: Style };
  const kind: FormeNodeKind = {
    type: 'Svg',
    width: props.width,
    height: props.height,
    content: props.content,
  };
  if (props.viewBox) (kind as { view_box?: string }).view_box = props.viewBox;

  return {
    kind,
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

// ─── Style mapping ──────────────────────────────────────────────────

const FLEX_DIRECTION_MAP: Record<string, string> = {
  'row': 'Row',
  'column': 'Column',
  'row-reverse': 'RowReverse',
  'column-reverse': 'ColumnReverse',
};

const JUSTIFY_CONTENT_MAP: Record<string, string> = {
  'flex-start': 'FlexStart',
  'flex-end': 'FlexEnd',
  'center': 'Center',
  'space-between': 'SpaceBetween',
  'space-around': 'SpaceAround',
  'space-evenly': 'SpaceEvenly',
};

const ALIGN_ITEMS_MAP: Record<string, string> = {
  'flex-start': 'FlexStart',
  'flex-end': 'FlexEnd',
  'center': 'Center',
  'stretch': 'Stretch',
  'baseline': 'Baseline',
};

const FLEX_WRAP_MAP: Record<string, string> = {
  'nowrap': 'NoWrap',
  'wrap': 'Wrap',
  'wrap-reverse': 'WrapReverse',
};

const ALIGN_CONTENT_MAP: Record<string, string> = {
  'flex-start': 'FlexStart',
  'flex-end': 'FlexEnd',
  'center': 'Center',
  'space-between': 'SpaceBetween',
  'space-around': 'SpaceAround',
  'space-evenly': 'SpaceEvenly',
  'stretch': 'Stretch',
};

const FONT_STYLE_MAP: Record<string, string> = {
  'normal': 'Normal',
  'italic': 'Italic',
  'oblique': 'Oblique',
};

const TEXT_ALIGN_MAP: Record<string, string> = {
  'left': 'Left',
  'right': 'Right',
  'center': 'Center',
  'justify': 'Justify',
};

const TEXT_DECORATION_MAP: Record<string, string> = {
  'none': 'None',
  'underline': 'Underline',
  'line-through': 'LineThrough',
};

const TEXT_TRANSFORM_MAP: Record<string, string> = {
  'none': 'None',
  'uppercase': 'Uppercase',
  'lowercase': 'Lowercase',
  'capitalize': 'Capitalize',
};

export function mapStyle(style?: Style): FormeStyle {
  if (!style) return {};

  const result: FormeStyle = {};

  // Dimensions
  if (style.width !== undefined) result.width = mapDimension(style.width);
  if (style.height !== undefined) result.height = mapDimension(style.height);
  if (style.minWidth !== undefined) result.minWidth = mapDimension(style.minWidth);
  if (style.minHeight !== undefined) result.minHeight = mapDimension(style.minHeight);
  if (style.maxWidth !== undefined) result.maxWidth = mapDimension(style.maxWidth);
  if (style.maxHeight !== undefined) result.maxHeight = mapDimension(style.maxHeight);

  // Edges (individual > axis > base)
  if (style.padding !== undefined || style.paddingTop !== undefined || style.paddingRight !== undefined || style.paddingBottom !== undefined || style.paddingLeft !== undefined || style.paddingHorizontal !== undefined || style.paddingVertical !== undefined) {
    const base = style.padding !== undefined ? expandEdges(style.padding) : { top: 0, right: 0, bottom: 0, left: 0 };
    const vt = style.paddingVertical ?? base.top;
    const vb = style.paddingVertical ?? base.bottom;
    const hl = style.paddingHorizontal ?? base.left;
    const hr = style.paddingHorizontal ?? base.right;
    result.padding = {
      top: style.paddingTop ?? vt,
      right: style.paddingRight ?? hr,
      bottom: style.paddingBottom ?? vb,
      left: style.paddingLeft ?? hl,
    };
  }
  if (style.margin !== undefined || style.marginTop !== undefined || style.marginRight !== undefined || style.marginBottom !== undefined || style.marginLeft !== undefined || style.marginHorizontal !== undefined || style.marginVertical !== undefined) {
    const base = style.margin !== undefined ? expandEdges(style.margin) : { top: 0, right: 0, bottom: 0, left: 0 };
    const vt = style.marginVertical ?? base.top;
    const vb = style.marginVertical ?? base.bottom;
    const hl = style.marginHorizontal ?? base.left;
    const hr = style.marginHorizontal ?? base.right;
    result.margin = {
      top: style.marginTop ?? vt,
      right: style.marginRight ?? hr,
      bottom: style.marginBottom ?? vb,
      left: style.marginLeft ?? hl,
    };
  }

  // Flex shorthand: flex: N → flexGrow: N, flexShrink: 1, flexBasis: 0
  if (style.flex !== undefined) {
    if (style.flexGrow === undefined) result.flexGrow = style.flex;
    if (style.flexShrink === undefined) result.flexShrink = 1;
    if (style.flexBasis === undefined) result.flexBasis = { Pt: 0 };
  }

  // Flex
  if (style.flexDirection !== undefined) result.flexDirection = FLEX_DIRECTION_MAP[style.flexDirection];
  if (style.justifyContent !== undefined) result.justifyContent = JUSTIFY_CONTENT_MAP[style.justifyContent];
  if (style.alignItems !== undefined) result.alignItems = ALIGN_ITEMS_MAP[style.alignItems];
  if (style.alignSelf !== undefined) result.alignSelf = ALIGN_ITEMS_MAP[style.alignSelf];
  if (style.flexWrap !== undefined) result.flexWrap = FLEX_WRAP_MAP[style.flexWrap];
  if (style.alignContent !== undefined) result.alignContent = ALIGN_CONTENT_MAP[style.alignContent];
  if (style.flexGrow !== undefined) result.flexGrow = style.flexGrow;
  if (style.flexShrink !== undefined) result.flexShrink = style.flexShrink;
  if (style.flexBasis !== undefined) result.flexBasis = mapDimension(style.flexBasis);
  if (style.gap !== undefined) result.gap = style.gap;
  if (style.rowGap !== undefined) result.rowGap = style.rowGap;
  if (style.columnGap !== undefined) result.columnGap = style.columnGap;

  // Typography
  if (style.fontFamily !== undefined) result.fontFamily = style.fontFamily;
  if (style.fontSize !== undefined) result.fontSize = style.fontSize;
  if (style.fontWeight !== undefined) {
    result.fontWeight = style.fontWeight === 'bold' ? 700 : style.fontWeight === 'normal' ? 400 : style.fontWeight;
  }
  if (style.fontStyle !== undefined) result.fontStyle = FONT_STYLE_MAP[style.fontStyle];
  if (style.lineHeight !== undefined) result.lineHeight = style.lineHeight;
  if (style.textAlign !== undefined) result.textAlign = TEXT_ALIGN_MAP[style.textAlign];
  if (style.letterSpacing !== undefined) result.letterSpacing = style.letterSpacing;
  if (style.textDecoration !== undefined) result.textDecoration = TEXT_DECORATION_MAP[style.textDecoration];
  if (style.textTransform !== undefined) result.textTransform = TEXT_TRANSFORM_MAP[style.textTransform];

  // Color
  if (style.color !== undefined) result.color = parseColor(style.color);
  if (style.backgroundColor !== undefined) result.backgroundColor = parseColor(style.backgroundColor);
  if (style.opacity !== undefined) result.opacity = style.opacity;

  // Border
  if (style.borderWidth !== undefined || style.borderTopWidth !== undefined || style.borderRightWidth !== undefined || style.borderBottomWidth !== undefined || style.borderLeftWidth !== undefined) {
    const base = style.borderWidth !== undefined ? expandEdgeValues(style.borderWidth) : { top: 0, right: 0, bottom: 0, left: 0 };
    result.borderWidth = {
      top: style.borderTopWidth ?? base.top,
      right: style.borderRightWidth ?? base.right,
      bottom: style.borderBottomWidth ?? base.bottom,
      left: style.borderLeftWidth ?? base.left,
    };
  }
  if (style.borderColor !== undefined || style.borderTopColor !== undefined || style.borderRightColor !== undefined || style.borderBottomColor !== undefined || style.borderLeftColor !== undefined) {
    const defaultColor = parseColor('#000000');
    let base = { top: defaultColor, right: defaultColor, bottom: defaultColor, left: defaultColor };
    if (typeof style.borderColor === 'string') {
      const c = parseColor(style.borderColor);
      base = { top: c, right: c, bottom: c, left: c };
    } else if (style.borderColor) {
      base = {
        top: parseColor(style.borderColor.top),
        right: parseColor(style.borderColor.right),
        bottom: parseColor(style.borderColor.bottom),
        left: parseColor(style.borderColor.left),
      };
    }
    result.borderColor = {
      top: style.borderTopColor ? parseColor(style.borderTopColor) : base.top,
      right: style.borderRightColor ? parseColor(style.borderRightColor) : base.right,
      bottom: style.borderBottomColor ? parseColor(style.borderBottomColor) : base.bottom,
      left: style.borderLeftColor ? parseColor(style.borderLeftColor) : base.left,
    };
  }
  if (style.borderRadius !== undefined || style.borderTopLeftRadius !== undefined || style.borderTopRightRadius !== undefined || style.borderBottomRightRadius !== undefined || style.borderBottomLeftRadius !== undefined) {
    const base = style.borderRadius !== undefined ? expandCorners(style.borderRadius) : { top_left: 0, top_right: 0, bottom_right: 0, bottom_left: 0 };
    result.borderRadius = {
      top_left: style.borderTopLeftRadius ?? base.top_left,
      top_right: style.borderTopRightRadius ?? base.top_right,
      bottom_right: style.borderBottomRightRadius ?? base.bottom_right,
      bottom_left: style.borderBottomLeftRadius ?? base.bottom_left,
    };
  }

  // Positioning
  if (style.position !== undefined) {
    result.position = style.position === 'absolute' ? 'Absolute' : 'Relative';
  }
  if (style.top !== undefined) result.top = style.top;
  if (style.right !== undefined) result.right = style.right;
  if (style.bottom !== undefined) result.bottom = style.bottom;
  if (style.left !== undefined) result.left = style.left;

  // Page behavior
  if (style.wrap !== undefined) result.wrap = style.wrap;
  if (style.breakBefore !== undefined) result.breakBefore = style.breakBefore;
  if (style.minWidowLines !== undefined) result.minWidowLines = style.minWidowLines;
  if (style.minOrphanLines !== undefined) result.minOrphanLines = style.minOrphanLines;

  return result;
}

export function mapDimension(val: number | string): FormeDimension {
  if (typeof val === 'number') {
    return { Pt: val };
  }
  if (val === 'auto') return 'Auto';
  const match = val.match(/^([0-9.]+)%$/);
  if (match) {
    return { Percent: parseFloat(match[1]) };
  }
  // Try to parse as a number (e.g. "100" without units)
  const num = parseFloat(val);
  if (!isNaN(num)) {
    return { Pt: num };
  }
  return 'Auto';
}

export function parseColor(hex: string): FormeColor {
  const h = hex.replace(/^#/, '');

  if (h.length === 3) {
    const r = parseInt(h[0] + h[0], 16) / 255;
    const g = parseInt(h[1] + h[1], 16) / 255;
    const b = parseInt(h[2] + h[2], 16) / 255;
    return { r, g, b, a: 1 };
  }

  if (h.length === 6) {
    const r = parseInt(h.slice(0, 2), 16) / 255;
    const g = parseInt(h.slice(2, 4), 16) / 255;
    const b = parseInt(h.slice(4, 6), 16) / 255;
    return { r, g, b, a: 1 };
  }

  if (h.length === 8) {
    const r = parseInt(h.slice(0, 2), 16) / 255;
    const g = parseInt(h.slice(2, 4), 16) / 255;
    const b = parseInt(h.slice(4, 6), 16) / 255;
    const a = parseInt(h.slice(6, 8), 16) / 255;
    return { r, g, b, a };
  }

  // Fallback: black
  return { r: 0, g: 0, b: 0, a: 1 };
}

export function expandEdges(val: number | Edges): FormeEdges {
  if (typeof val === 'number') {
    return { top: val, right: val, bottom: val, left: val };
  }
  return { top: val.top, right: val.right, bottom: val.bottom, left: val.left };
}

function expandEdgeValues(val: number | Edges): FormeEdgeValues<number> {
  if (typeof val === 'number') {
    return { top: val, right: val, bottom: val, left: val };
  }
  return { top: val.top, right: val.right, bottom: val.bottom, left: val.left };
}

export function expandCorners(val: number | Corners): FormeCornerValues {
  if (typeof val === 'number') {
    return { top_left: val, top_right: val, bottom_right: val, bottom_left: val };
  }
  return {
    top_left: val.topLeft,
    top_right: val.topRight,
    bottom_right: val.bottomRight,
    bottom_left: val.bottomLeft,
  };
}

function mapColumnWidth(w: ColumnDef['width']): FormeColumnWidth {
  if (w === 'auto') return 'Auto';
  if ('fraction' in w) return { Fraction: w.fraction };
  if ('fixed' in w) return { Fixed: w.fixed };
  return 'Auto';
}

// ─── Font merging ─────────────────────────────────────────────────

function normalizeFontWeight(w?: number | string): number {
  if (w === undefined || w === 'normal') return 400;
  if (w === 'bold') return 700;
  return typeof w === 'number' ? w : (parseInt(w, 10) || 400);
}

function fontKey(family: string, weight: number, italic: boolean): string {
  return `${family}:${weight}:${italic}`;
}

function mergeFonts(
  globalFonts: FontRegistration[],
  docFonts?: FontRegistration[],
): FormeFont[] {
  const map = new Map<string, FormeFont>();

  for (const f of globalFonts) {
    const weight = normalizeFontWeight(f.fontWeight);
    const italic = f.fontStyle === 'italic' || f.fontStyle === 'oblique';
    const key = fontKey(f.family, weight, italic);
    map.set(key, { family: f.family, src: f.src, weight, italic });
  }

  if (docFonts) {
    for (const f of docFonts) {
      const weight = normalizeFontWeight(f.fontWeight);
      const italic = f.fontStyle === 'italic' || f.fontStyle === 'oblique';
      const key = fontKey(f.family, weight, italic);
      map.set(key, { family: f.family, src: f.src, weight, italic });
    }
  }

  return Array.from(map.values());
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
  if (element.type !== Document) {
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

  if (mergedFonts.length > 0) {
    result.fonts = mergedFonts;
  }

  return result;
}

function serializeTemplatePage(element: ReactElement): Record<string, unknown> {
  const props = element.props as { size?: string | { width: number; height: number }; margin?: number | Edges; children?: unknown };

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
    return getExpr(child);
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
  if (element.type === PageBreak) {
    return { kind: { type: 'PageBreak' }, style: {}, children: [] };
  }
  if (element.type === Page) {
    validateNesting('Page', parent);
    return serializeTemplatePage(element);
  }

  // Unknown function component — call it
  if (typeof element.type === 'function') {
    const result = (element.type as (props: Record<string, unknown>) => unknown)(element.props as Record<string, unknown>);
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
