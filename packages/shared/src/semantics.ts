/**
 * Framework-neutral constants for the semantic components (headings,
 * inline formatting, lists). Each adapter (react, svelte) walks its own
 * element tree, but the default styles and the list-marker wire mapping
 * must be identical so the adapters cannot drift.
 */

import type { Style } from './types.js';
import type { FormeListMarkerType } from './types.js';

// Inline formatting components produce TextRuns with these default styles
// (merged under any user-supplied style, so user style wins).
export const STRONG_DEFAULTS: Style = { fontWeight: 700 };
export const EM_DEFAULTS: Style = { fontStyle: 'italic' };
export const CODE_DEFAULTS: Style = {
  fontFamily: 'Courier',
  backgroundColor: '#F4F4F5',
};
export const LINK_DEFAULTS: Style = {
  color: '#2563EB',
  textDecoration: 'underline',
};

// Default style per heading level. Tuned for typical document layout —
// users override individual properties via `style` on the heading element.
// margin top/bottom are in points; font sizes are points.
export const HEADING_DEFAULTS: Record<1 | 2 | 3 | 4 | 5 | 6, Style> = {
  1: { fontSize: 32, fontWeight: 700, marginTop: 24, marginBottom: 16 },
  2: { fontSize: 24, fontWeight: 700, marginTop: 20, marginBottom: 14 },
  3: { fontSize: 20, fontWeight: 600, marginTop: 16, marginBottom: 12 },
  4: { fontSize: 18, fontWeight: 600, marginTop: 14, marginBottom: 10 },
  5: { fontSize: 16, fontWeight: 600, marginTop: 12, marginBottom: 8 },
  6: { fontSize: 14, fontWeight: 600, marginTop: 10, marginBottom: 6 },
};

/** CSS `list-style-type`-shaped string → engine wire enum value. */
export function mapListMarker(
  marker: string | undefined,
  defaultValue: FormeListMarkerType,
): FormeListMarkerType {
  switch (marker) {
    case 'disc': return 'disc';
    case 'circle': return 'circle';
    case 'square': return 'square';
    case 'none': return 'none';
    case 'decimal': return 'decimal';
    case 'lower-alpha': return 'lowerAlpha';
    case 'upper-alpha': return 'upperAlpha';
    case 'lower-roman': return 'lowerRoman';
    case 'upper-roman': return 'upperRoman';
    default: return defaultValue;
  }
}
