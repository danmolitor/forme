import type {
  Style,
  Edges,
  Corners,
  ColumnDef,
  GridTrackSize,
  FormeStyle,
  FormeEdges,
  FormeMarginEdges,
  FormeColumnWidth,
  FormeDimension,
  FormeColor,
  FormeBoxShadow,
  FormeBackground,
  FormeGradientStop,
  FormeEdgeValues,
  FormeCornerValues,
  FormeGridTrackSize,
  FormeGridPlacement,
  FormeTransformOp,
} from './types.js';

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

const HYPHENS_MAP: Record<string, string> = {
  'none': 'none',
  'manual': 'manual',
  'auto': 'auto',
};

const TEXT_OVERFLOW_MAP: Record<string, string> = {
  'wrap': 'Wrap',
  'ellipsis': 'Ellipsis',
  'clip': 'Clip',
};

const LINE_BREAKING_MAP: Record<string, string> = {
  'optimal': 'optimal',
  'greedy': 'greedy',
};

const OVERFLOW_MAP: Record<string, string> = {
  'visible': 'Visible',
  'hidden': 'Hidden',
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
    const base: FormeMarginEdges = style.margin !== undefined ? expandMarginEdges(style.margin) : { top: 0, right: 0, bottom: 0, left: 0 };
    const vt: number | 'auto' = style.marginVertical ?? base.top;
    const vb: number | 'auto' = style.marginVertical ?? base.bottom;
    const hl: number | 'auto' = style.marginHorizontal ?? base.left;
    const hr: number | 'auto' = style.marginHorizontal ?? base.right;
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

  // Display mode
  if (style.display !== undefined) {
    result.display = style.display === 'grid' ? 'Grid' : 'Flex';
  }

  // CSS Grid
  if (style.gridTemplateColumns !== undefined) {
    result.gridTemplateColumns = parseGridTemplate(style.gridTemplateColumns);
  }
  if (style.gridTemplateRows !== undefined) {
    result.gridTemplateRows = parseGridTemplate(style.gridTemplateRows);
  }
  if (style.gridAutoRows !== undefined) {
    result.gridAutoRows = mapGridTrack(style.gridAutoRows);
  }
  if (style.gridAutoColumns !== undefined) {
    result.gridAutoColumns = mapGridTrack(style.gridAutoColumns);
  }
  // Grid placement (individual props → single gridPlacement object)
  if (style.gridColumnStart !== undefined || style.gridColumnEnd !== undefined ||
      style.gridRowStart !== undefined || style.gridRowEnd !== undefined ||
      style.gridColumnSpan !== undefined || style.gridRowSpan !== undefined) {
    const placement: FormeGridPlacement = {};
    if (style.gridColumnStart !== undefined) placement.columnStart = style.gridColumnStart;
    if (style.gridColumnEnd !== undefined) placement.columnEnd = style.gridColumnEnd;
    if (style.gridRowStart !== undefined) placement.rowStart = style.gridRowStart;
    if (style.gridRowEnd !== undefined) placement.rowEnd = style.gridRowEnd;
    if (style.gridColumnSpan !== undefined) placement.columnSpan = style.gridColumnSpan;
    if (style.gridRowSpan !== undefined) placement.rowSpan = style.gridRowSpan;
    result.gridPlacement = placement;
  }

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
  if (style.wordSpacing !== undefined) result.wordSpacing = style.wordSpacing;
  if (style.boxShadow !== undefined) {
    const parsed = parseBoxShadow(style.boxShadow);
    if (parsed) result.boxShadow = parsed;
  }
  if (style.transform !== undefined) {
    const parsed = parseTransform(style.transform);
    if (parsed && parsed.length > 0) result.transform = parsed;
  }
  if (style.transformOrigin !== undefined) {
    const parsed = parseTransformOrigin(style.transformOrigin);
    if (parsed) result.transformOrigin = parsed;
  }
  if (style.textDecoration !== undefined) result.textDecoration = TEXT_DECORATION_MAP[style.textDecoration];
  if (style.textTransform !== undefined) result.textTransform = TEXT_TRANSFORM_MAP[style.textTransform];
  if (style.hyphens !== undefined) result.hyphens = HYPHENS_MAP[style.hyphens];
  if (style.lang !== undefined) result.lang = style.lang;
  if (style.direction !== undefined) result.direction = style.direction;
  if (style.textOverflow !== undefined) result.textOverflow = TEXT_OVERFLOW_MAP[style.textOverflow];
  if (style.lineBreaking !== undefined) result.lineBreaking = LINE_BREAKING_MAP[style.lineBreaking];
  if (style.overflow !== undefined) result.overflow = OVERFLOW_MAP[style.overflow];

  // Color
  if (style.color !== undefined) result.color = parseColor(style.color);
  if (style.backgroundColor !== undefined) result.backgroundColor = parseColor(style.backgroundColor);
  if (style.background !== undefined) {
    const parsed = parseBackground(style.background);
    if (parsed) {
      if (parsed.type === 'color') {
        // Solid color string in `background`: route to backgroundColor for
        // engine compatibility (Background::Color also works, but
        // backgroundColor is the canonical solid path).
        if (result.backgroundColor === undefined) result.backgroundColor = parsed.value;
      } else {
        result.background = parsed;
      }
    }
  }
  if (style.opacity !== undefined) result.opacity = style.opacity;

  // Border — cascade: border < borderTop/Right/Bottom/Left < borderWidth/borderColor < borderTopWidth/borderTopColor
  // Step 1: Parse string shorthands into intermediate per-side values
  let shortWidth: FormeEdgeValues<number | undefined> = { top: undefined, right: undefined, bottom: undefined, left: undefined };
  let shortColor: FormeEdgeValues<FormeColor | undefined> = { top: undefined, right: undefined, bottom: undefined, left: undefined };

  if (style.border !== undefined) {
    const parsed = parseBorderString(style.border);
    if (parsed.width !== undefined) shortWidth = { top: parsed.width, right: parsed.width, bottom: parsed.width, left: parsed.width };
    if (parsed.color !== undefined) shortColor = { top: parsed.color, right: parsed.color, bottom: parsed.color, left: parsed.color };
  }

  // Per-side string shorthands override all-side shorthand
  for (const [side, prop] of [['top', 'borderTop'], ['right', 'borderRight'], ['bottom', 'borderBottom'], ['left', 'borderLeft']] as const) {
    const val = style[prop];
    if (val === undefined) continue;
    if (typeof val === 'number') {
      shortWidth[side] = val;
    } else {
      const parsed = parseBorderString(val);
      if (parsed.width !== undefined) shortWidth[side] = parsed.width;
      if (parsed.color !== undefined) shortColor[side] = parsed.color;
    }
  }

  // Step 2: Build borderWidth — existing borderWidth/borderTopWidth override shorthands
  const hasBorderWidth = style.borderWidth !== undefined || style.borderTopWidth !== undefined || style.borderRightWidth !== undefined || style.borderBottomWidth !== undefined || style.borderLeftWidth !== undefined;
  const hasShortWidth = shortWidth.top !== undefined || shortWidth.right !== undefined || shortWidth.bottom !== undefined || shortWidth.left !== undefined;
  if (hasBorderWidth || hasShortWidth) {
    const base = style.borderWidth !== undefined
      ? expandEdgeValues(style.borderWidth)
      : { top: shortWidth.top ?? 0, right: shortWidth.right ?? 0, bottom: shortWidth.bottom ?? 0, left: shortWidth.left ?? 0 };
    result.borderWidth = {
      top: style.borderTopWidth ?? base.top,
      right: style.borderRightWidth ?? base.right,
      bottom: style.borderBottomWidth ?? base.bottom,
      left: style.borderLeftWidth ?? base.left,
    };
  }

  // Step 3: Build borderColor — existing borderColor/borderTopColor override shorthands
  const hasBorderColor = style.borderColor !== undefined || style.borderTopColor !== undefined || style.borderRightColor !== undefined || style.borderBottomColor !== undefined || style.borderLeftColor !== undefined;
  const hasShortColor = shortColor.top !== undefined || shortColor.right !== undefined || shortColor.bottom !== undefined || shortColor.left !== undefined;
  if (hasBorderColor || hasShortColor) {
    const defaultColor = parseColor('#000000');
    let base = {
      top: shortColor.top ?? defaultColor,
      right: shortColor.right ?? defaultColor,
      bottom: shortColor.bottom ?? defaultColor,
      left: shortColor.left ?? defaultColor,
    };
    if (typeof style.borderColor === 'string') {
      const c = parseColor(style.borderColor);
      base = { top: c, right: c, bottom: c, left: c };
    } else if (style.borderColor && typeof style.borderColor === 'object') {
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

// ─── Grid helpers ───────────────────────────────────────────────────

/** Convert a single GridTrackSize to the Forme JSON format. */
function mapGridTrack(track: GridTrackSize): FormeGridTrackSize {
  if (typeof track === 'number') return { Pt: track };
  if (track === 'auto') return 'Auto';
  if (typeof track === 'string') {
    const frMatch = track.match(/^([0-9.]+)fr$/);
    if (frMatch) return { Fr: parseFloat(frMatch[1]) };
    // Try numeric string
    const num = parseFloat(track);
    if (!isNaN(num)) return { Pt: num };
    return 'Auto';
  }
  if (typeof track === 'object' && 'min' in track && 'max' in track) {
    return { MinMax: [mapGridTrack(track.min), mapGridTrack(track.max)] };
  }
  return 'Auto';
}

/**
 * Expand `repeat(N, tracks)` in a grid template string.
 * E.g. `"repeat(3, 1fr)"` → `"1fr 1fr 1fr"`
 *       `"200 repeat(2, 1fr) 200"` → `"200 1fr 1fr 200"`
 */
function expandRepeat(input: string): string {
  return input.replace(/repeat\(\s*(\d+)\s*,\s*([^)]+)\)/g, (_match, count, tracks) => {
    return (tracks.trim() + ' ').repeat(parseInt(count, 10)).trim();
  });
}

/**
 * Parse a grid template string shorthand into an array of FormeGridTrackSize.
 * E.g. `"1fr 2fr 200"` → `[{Fr:1}, {Fr:2}, {Pt:200}]`
 * Supports `repeat(N, tracks)` syntax.
 */
function parseGridTemplate(value: string | GridTrackSize[]): FormeGridTrackSize[] {
  if (Array.isArray(value)) {
    return value.map(mapGridTrack);
  }
  const expanded = expandRepeat(value);
  return expanded.split(/\s+/).filter(Boolean).map((token) => {
    if (token === 'auto') return 'Auto' as FormeGridTrackSize;
    const frMatch = token.match(/^([0-9.]+)fr$/);
    if (frMatch) return { Fr: parseFloat(frMatch[1]) } as FormeGridTrackSize;
    const num = parseFloat(token);
    if (!isNaN(num)) return { Pt: num } as FormeGridTrackSize;
    return 'Auto' as FormeGridTrackSize;
  });
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
  const s = hex.trim();

  // rgba(r, g, b, a)
  const rgbaMatch = s.match(/^rgba\(\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\s*\)$/);
  if (rgbaMatch) {
    return {
      r: parseFloat(rgbaMatch[1]) / 255,
      g: parseFloat(rgbaMatch[2]) / 255,
      b: parseFloat(rgbaMatch[3]) / 255,
      a: parseFloat(rgbaMatch[4]),
    };
  }

  // rgb(r, g, b)
  const rgbMatch = s.match(/^rgb\(\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\s*,\s*(\d+(?:\.\d+)?)\s*\)$/);
  if (rgbMatch) {
    return {
      r: parseFloat(rgbMatch[1]) / 255,
      g: parseFloat(rgbMatch[2]) / 255,
      b: parseFloat(rgbMatch[3]) / 255,
      a: 1,
    };
  }

  const h = s.replace(/^#/, '');

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

/**
 * Parse a CSS-like `transform` string into the engine's TransformOp[].
 *
 * Accepted syntax (space- or comma-separated ops, in any order):
 *   `rotate(45deg)`, `rotate(-15deg)`, `rotate(0.5turn)`, `rotate(1.2rad)`
 *   `scale(1.5)`, `scale(2, 0.5)`
 *   `translate(10, -4)`, `translate(10px, -4pt)`
 *
 * Lengths are interpreted as points (px/pt suffix accepted and ignored;
 * Forme is a print-first engine — 1 point = 1 point). Returns `null` on
 * unparseable input (the property is silently dropped, matching the
 * boxShadow precedent).
 */
function parseTransform(val: string): FormeTransformOp[] | null {
  const ops: FormeTransformOp[] = [];
  // Match `name(args)` pairs. Names: ASCII letters. Args: anything between
  // parentheses (no nesting in this grammar).
  const re = /([a-zA-Z]+)\s*\(([^)]*)\)/g;
  let m: RegExpExecArray | null;
  let matched = false;
  while ((m = re.exec(val)) !== null) {
    matched = true;
    const name = m[1].toLowerCase();
    const args = m[2]
      .split(/[,\s]+/)
      .map((s) => s.trim())
      .filter((s) => s.length > 0);

    if (name === 'rotate' || name === 'rotatez') {
      if (args.length !== 1) return null;
      const deg = parseAngle(args[0]);
      if (deg === null) return null;
      ops.push({ type: 'rotate', deg });
    } else if (name === 'scale') {
      if (args.length === 0 || args.length > 2) return null;
      const x = parseFloat(args[0]);
      if (Number.isNaN(x)) return null;
      const y = args.length === 2 ? parseFloat(args[1]) : x;
      if (Number.isNaN(y)) return null;
      ops.push({ type: 'scale', x, y });
    } else if (name === 'scalex') {
      if (args.length !== 1) return null;
      const x = parseFloat(args[0]);
      if (Number.isNaN(x)) return null;
      ops.push({ type: 'scale', x, y: 1 });
    } else if (name === 'scaley') {
      if (args.length !== 1) return null;
      const y = parseFloat(args[0]);
      if (Number.isNaN(y)) return null;
      ops.push({ type: 'scale', x: 1, y });
    } else if (name === 'translate') {
      if (args.length === 0 || args.length > 2) return null;
      const x = parseLengthPt(args[0]);
      if (x === null) return null;
      const y = args.length === 2 ? parseLengthPt(args[1]) : 0;
      if (y === null) return null;
      ops.push({ type: 'translate', x, y });
    } else if (name === 'translatex') {
      if (args.length !== 1) return null;
      const x = parseLengthPt(args[0]);
      if (x === null) return null;
      ops.push({ type: 'translate', x, y: 0 });
    } else if (name === 'translatey') {
      if (args.length !== 1) return null;
      const y = parseLengthPt(args[0]);
      if (y === null) return null;
      ops.push({ type: 'translate', x: 0, y });
    } else {
      // Unknown op (e.g. skew, matrix) — reject the whole transform so
      // users notice rather than getting silently-half-applied transforms.
      return null;
    }
  }
  if (!matched) return null;
  return ops;
}

/**
 * Parse an angle in `deg` / `rad` / `turn` (no unit defaults to deg).
 * Returns the angle in DEGREES, or null on parse failure.
 */
function parseAngle(s: string): number | null {
  const t = s.trim().toLowerCase();
  let mult = 1;
  let body = t;
  if (t.endsWith('deg')) {
    body = t.slice(0, -3);
  } else if (t.endsWith('rad')) {
    body = t.slice(0, -3);
    mult = 180 / Math.PI;
  } else if (t.endsWith('turn')) {
    body = t.slice(0, -4);
    mult = 360;
  }
  const n = parseFloat(body);
  if (Number.isNaN(n)) return null;
  return n * mult;
}

/**
 * Parse a length value — accepts a bare number or `<n>px`/`<n>pt` and
 * returns the value in points. Forme is print-first; px/pt are treated
 * as the same unit at this layer.
 */
function parseLengthPt(s: string): number | null {
  const t = s.trim().toLowerCase();
  const body = t.endsWith('px') || t.endsWith('pt') ? t.slice(0, -2) : t;
  const n = parseFloat(body);
  return Number.isNaN(n) ? null : n;
}

/**
 * Parse a `transformOrigin` value into a `[x, y]` tuple of fractions
 * (0.0–1.0). Accepts the user-side tuple shape directly, or a CSS-like
 * string with two tokens (`"50% 50%"`, `"0% 100%"`, `"center top"`,
 * `"left bottom"`, etc.). Returns null on unparseable input.
 */
function parseTransformOrigin(
  val: string | [number, number],
): [number, number] | null {
  if (Array.isArray(val)) {
    const [x, y] = val;
    if (typeof x !== 'number' || typeof y !== 'number') return null;
    return [x, y];
  }
  const tokens = val.trim().split(/\s+/).filter((t) => t.length > 0);
  if (tokens.length === 0 || tokens.length > 2) return null;
  const x = parseOriginToken(tokens[0], 'x');
  if (x === null) return null;
  const y = tokens.length === 2 ? parseOriginToken(tokens[1], 'y') : 0.5;
  if (y === null) return null;
  return [x, y];
}

function parseOriginToken(token: string, axis: 'x' | 'y'): number | null {
  const t = token.toLowerCase();
  if (t === 'center') return 0.5;
  if (axis === 'x') {
    if (t === 'left') return 0;
    if (t === 'right') return 1;
  } else {
    if (t === 'top') return 0;
    if (t === 'bottom') return 1;
  }
  if (t.endsWith('%')) {
    const n = parseFloat(t.slice(0, -1));
    return Number.isNaN(n) ? null : n / 100;
  }
  return null;
}

/**
 * Parse a `boxShadow` value (object form or CSS-like string
 * `"offsetX offsetY blur color"`) into the engine's FormeBoxShadow shape.
 * Returns null on malformed input. v1 ignores blur but parses it.
 */
function parseBoxShadow(
  val: string | { offsetX: number; offsetY: number; blur?: number; color: string },
): FormeBoxShadow | null {
  if (typeof val === 'object') {
    const c = parseColor(val.color);
    return {
      offsetX: val.offsetX,
      offsetY: val.offsetY,
      blur: val.blur ?? 0,
      color: c,
    };
  }
  // String form: "offsetX offsetY blur color".
  // Split on whitespace, but preserve any rgba(...)/rgb(...) parens.
  const tokens: string[] = [];
  let depth = 0;
  let buf = '';
  for (const ch of val.trim()) {
    if (ch === '(') depth += 1;
    if (ch === ')') depth = Math.max(0, depth - 1);
    if (/\s/.test(ch) && depth === 0) {
      if (buf) {
        tokens.push(buf);
        buf = '';
      }
    } else {
      buf += ch;
    }
  }
  if (buf) tokens.push(buf);
  if (tokens.length < 4) return null;
  const offsetX = parseFloat(tokens[0]);
  const offsetY = parseFloat(tokens[1]);
  const blur = parseFloat(tokens[2]);
  if (Number.isNaN(offsetX) || Number.isNaN(offsetY) || Number.isNaN(blur)) return null;
  const color = parseColor(tokens[3]);
  return { offsetX, offsetY, blur, color };
}

/**
 * Parse a CSS `background` value. Supports three forms:
 *   - `linear-gradient(<angle>, <stop>, <stop>, ...)` — CSS angle conventions
 *     (`0deg` = bottom→top, `90deg` = left→right, `180deg` = top→bottom).
 *     Angle is optional; defaults to `180deg` (top→bottom). Side keywords
 *     (`to bottom`, `to right`, etc.) also supported.
 *   - `radial-gradient(circle, <stop>, <stop>, ...)` — `circle` is the only
 *     shape in v1. The `circle` keyword is optional.
 *   - solid color (`#abc`, `rgb(...)`, `rgba(...)`) — falls through to a
 *     `Color`-typed background, which the caller routes to `backgroundColor`.
 *
 * v1 supports exactly 2 stops; gradients with 3+ stops are flattened to
 * the first and last stop (the engine's v1 ShadingType 2 only renders 2
 * colors). Multi-stop support is planned via PDF Type 3 stitching.
 *
 * Returns null on parse failure (e.g. malformed gradient string with no
 * usable color tokens) so the caller can omit the property.
 */
function parseBackground(val: string): FormeBackground | null {
  const s = val.trim();

  // linear-gradient(...)
  const linearMatch = s.match(/^linear-gradient\s*\(\s*([\s\S]*)\s*\)$/i);
  if (linearMatch) {
    const inner = linearMatch[1];
    const parts = splitGradientArgs(inner);
    if (parts.length === 0) return null;

    let angleDeg = 180;
    let stopParts = parts;
    const first = parts[0].trim();
    const angleParsed = parseGradientAngle(first);
    if (angleParsed !== null) {
      angleDeg = angleParsed;
      stopParts = parts.slice(1);
    }
    const stops = parseGradientStops(stopParts);
    if (stops.length < 2) return null;
    return { type: 'linear', angleDeg, stops };
  }

  // radial-gradient(...)
  const radialMatch = s.match(/^radial-gradient\s*\(\s*([\s\S]*)\s*\)$/i);
  if (radialMatch) {
    const inner = radialMatch[1];
    const parts = splitGradientArgs(inner);
    if (parts.length === 0) return null;
    let stopParts = parts;
    // Optional shape keyword (`circle`, `ellipse`) — only `circle` honored
    // here; `ellipse` strings parse but render as a circle.
    const first = parts[0].trim().toLowerCase();
    if (first === 'circle' || first === 'ellipse' || first.startsWith('circle ') || first.startsWith('ellipse ')) {
      stopParts = parts.slice(1);
    }
    const stops = parseGradientStops(stopParts);
    if (stops.length < 2) return null;
    return { type: 'radial', stops };
  }

  // Solid color fallback. parseColor never throws; on garbage input it
  // returns black, so check that the input looks color-shaped first to
  // avoid silently turning a typo'd gradient into a black background.
  if (/^(#|rgb\(|rgba\()/i.test(s)) {
    return { type: 'color', value: parseColor(s) };
  }
  return null;
}

/**
 * Split a gradient's interior comma-separated arguments, respecting parens
 * so `rgba(0, 0, 0, 0.5)` doesn't get split mid-color.
 */
function splitGradientArgs(inner: string): string[] {
  const parts: string[] = [];
  let depth = 0;
  let buf = '';
  for (const ch of inner) {
    if (ch === '(') depth += 1;
    else if (ch === ')') depth = Math.max(0, depth - 1);
    if (ch === ',' && depth === 0) {
      parts.push(buf);
      buf = '';
    } else {
      buf += ch;
    }
  }
  if (buf.trim()) parts.push(buf);
  return parts.map((p) => p.trim()).filter((p) => p.length > 0);
}

/**
 * Parse a CSS gradient angle token. Returns degrees, or null if the token
 * isn't an angle.
 *
 * Forms supported:
 *   - `<n>deg` (e.g. `135deg`)
 *   - `<n>turn` (e.g. `0.5turn`)
 *   - `<n>rad` / `<n>grad`
 *   - `to <side>` keywords: `to top` (0), `to right` (90), `to bottom` (180), `to left` (270),
 *     and the four diagonal forms (`to top right`, etc.).
 */
function parseGradientAngle(token: string): number | null {
  const t = token.trim().toLowerCase();
  const degMatch = t.match(/^(-?\d+(?:\.\d+)?)deg$/);
  if (degMatch) return parseFloat(degMatch[1]);
  const turnMatch = t.match(/^(-?\d+(?:\.\d+)?)turn$/);
  if (turnMatch) return parseFloat(turnMatch[1]) * 360;
  const radMatch = t.match(/^(-?\d+(?:\.\d+)?)rad$/);
  if (radMatch) return (parseFloat(radMatch[1]) * 180) / Math.PI;
  const gradMatch = t.match(/^(-?\d+(?:\.\d+)?)grad$/);
  if (gradMatch) return parseFloat(gradMatch[1]) * 0.9;
  if (t === 'to top') return 0;
  if (t === 'to right') return 90;
  if (t === 'to bottom') return 180;
  if (t === 'to left') return 270;
  if (t === 'to top right' || t === 'to right top') return 45;
  if (t === 'to bottom right' || t === 'to right bottom') return 135;
  if (t === 'to bottom left' || t === 'to left bottom') return 225;
  if (t === 'to top left' || t === 'to left top') return 315;
  return null;
}

/**
 * Parse a list of gradient color stops. Each stop is `<color>` or
 * `<color> <position>`. Position can be `<n>%` (CSS) or `<n>` (treated as
 * a 0..1 fraction). Stops without explicit positions get evenly distributed
 * positions matching CSS defaults: first at 0, last at 1, intermediate
 * stops linearly interpolated.
 */
function parseGradientStops(parts: string[]): FormeGradientStop[] {
  if (parts.length === 0) return [];
  const positions: (number | null)[] = [];
  const colors: { r: number; g: number; b: number; a: number }[] = [];
  for (const p of parts) {
    // Find the last whitespace-separated token; if it's a position
    // (`50%` or `0.5`), treat it as such, else everything is the color.
    const trimmed = p.trim();
    const tokens = splitColorAndPosition(trimmed);
    if (!tokens) continue;
    colors.push(parseColor(tokens.color));
    positions.push(tokens.position);
  }
  if (colors.length === 0) return [];

  // Fill in missing positions with CSS defaults.
  if (positions[0] === null) positions[0] = 0;
  if (positions[positions.length - 1] === null) positions[positions.length - 1] = 1;
  for (let i = 1; i < positions.length - 1; i += 1) {
    if (positions[i] === null) {
      // Linear interpolate between previous known and next known.
      let prev = i - 1;
      while (prev >= 0 && positions[prev] === null) prev -= 1;
      let next = i + 1;
      while (next < positions.length && positions[next] === null) next += 1;
      const p0 = positions[prev] ?? 0;
      const p1 = positions[next] ?? 1;
      positions[i] = p0 + ((p1 - p0) * (i - prev)) / (next - prev);
    }
  }
  return colors.map((color, i) => ({
    position: Math.max(0, Math.min(1, positions[i] as number)),
    color,
  }));
}

/**
 * Split a stop string like `"#fff 50%"` into color + position.
 * Position can be percentage or fraction; null if not specified.
 */
function splitColorAndPosition(s: string): { color: string; position: number | null } | null {
  // The position token, if present, is the last whitespace-separated piece
  // and matches `<number>%` or a bare number. Splitting on whitespace
  // respects parens so `rgba(...)` is not chopped up.
  const tokens: string[] = [];
  let depth = 0;
  let buf = '';
  for (const ch of s) {
    if (ch === '(') depth += 1;
    else if (ch === ')') depth = Math.max(0, depth - 1);
    if (/\s/.test(ch) && depth === 0) {
      if (buf) {
        tokens.push(buf);
        buf = '';
      }
    } else {
      buf += ch;
    }
  }
  if (buf) tokens.push(buf);
  if (tokens.length === 0) return null;
  const last = tokens[tokens.length - 1];
  const pctMatch = last.match(/^(-?\d+(?:\.\d+)?)%$/);
  if (pctMatch) {
    return { color: tokens.slice(0, -1).join(' '), position: parseFloat(pctMatch[1]) / 100 };
  }
  const fracMatch = last.match(/^(-?\d+(?:\.\d+)?)$/);
  if (fracMatch && tokens.length > 1) {
    return { color: tokens.slice(0, -1).join(' '), position: parseFloat(fracMatch[1]) };
  }
  return { color: tokens.join(' '), position: null };
}

/**
 * Parse a CSS-style 1-4 value edge shorthand.
 * Accepts: `"8"`, `"8 16"`, `"8 16 24"`, `"8 16 24 32"` (with optional `px` suffix).
 * Also accepts number arrays: `[8]`, `[8, 16]`, `[8, 16, 24]`, `[8, 16, 24, 32]`.
 */
function parseCSSEdges(val: string | number[]): FormeEdges {
  const values: number[] = Array.isArray(val)
    ? val
    : val.trim().split(/\s+/).map(s => parseFloat(s.replace(/px$/i, '')));

  switch (values.length) {
    case 1: return { top: values[0], right: values[0], bottom: values[0], left: values[0] };
    case 2: return { top: values[0], right: values[1], bottom: values[0], left: values[1] };
    case 3: return { top: values[0], right: values[1], bottom: values[2], left: values[1] };
    default: return { top: values[0], right: values[1], bottom: values[2], left: values[3] };
  }
}

const BORDER_STYLE_KEYWORDS = new Set([
  'solid', 'dashed', 'dotted', 'double', 'groove', 'ridge', 'inset', 'outset', 'none', 'hidden',
]);

/**
 * Parse a CSS border shorthand string like `"1px solid #000"`.
 * Returns extracted width and/or color. Style keywords are recognized but ignored.
 */
function parseBorderString(val: string): { width?: number; color?: FormeColor } {
  const tokens = val.trim().split(/\s+/);
  let width: number | undefined;
  let color: FormeColor | undefined;

  for (const token of tokens) {
    const lower = token.toLowerCase();
    if (BORDER_STYLE_KEYWORDS.has(lower)) continue;

    const num = parseFloat(lower.replace(/px$/i, ''));
    if (!isNaN(num) && /^[\d.]/.test(lower)) {
      width = num;
    } else {
      color = parseColor(token);
    }
  }

  return { width, color };
}

export function expandEdges(val: number | string | number[] | Edges): FormeEdges {
  if (typeof val === 'number') {
    return { top: val, right: val, bottom: val, left: val };
  }
  if (typeof val === 'string' || Array.isArray(val)) {
    return parseCSSEdges(val);
  }
  return { top: val.top, right: val.right, bottom: val.bottom, left: val.left };
}

/** Expand margin edges, preserving 'auto' string values. */
function expandMarginEdges(val: number | string | number[] | Edges): FormeMarginEdges {
  if (typeof val === 'number') {
    return { top: val, right: val, bottom: val, left: val };
  }
  if (typeof val === 'string') {
    if (val === 'auto') {
      return { top: 'auto', right: 'auto', bottom: 'auto', left: 'auto' };
    }
    const edges = parseCSSEdges(val);
    return edges;
  }
  if (Array.isArray(val)) {
    return parseCSSEdges(val);
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

export function mapColumnWidth(w: ColumnDef['width']): FormeColumnWidth {
  if (w === 'auto') return 'Auto';
  if ('fraction' in w) return { Fraction: w.fraction };
  if ('fixed' in w) return { Fixed: w.fixed };
  return 'Auto';
}
