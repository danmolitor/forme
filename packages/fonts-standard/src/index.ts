/**
 * @formepdf/fonts-standard
 *
 * Metric-compatible, embeddable replacements for the 14 standard PDF fonts,
 * for producing accessible (PDF/UA) and archival (PDF/A) documents — those
 * profiles require every font embedded, which the non-embedded base-14 fonts
 * are not.
 *
 * The Liberation family (Sans/Serif/Mono, SIL OFL 1.1) is metric-compatible
 * with Helvetica/Times/Courier by design, so substituting it changes nothing
 * about layout — Forme lays out on the standard AFM metrics and swaps only
 * the embedded glyph program at write time.
 *
 * The library carries the font bytes inline (base64) and returns them as
 * `Uint8Array` buffers — no file IO — so it works in Node, WASM, and the
 * browser. The fonts are redistributed unmodified under the OFL; see OFL.txt.
 */
import { FONT_DATA_BASE64 } from './generated/font-data.js';

/** A font registration compatible with `@formepdf/*`'s `Font.register`
 *  (structural — this package intentionally has no Forme dependencies). */
export interface FontSpec {
  family: string;
  src: Uint8Array;
  fontWeight: number;
  fontStyle: 'normal' | 'italic';
}

interface FontEntry {
  file: string;
  family: string;
  fontWeight: number;
  fontStyle: 'normal' | 'italic';
}

const ENTRIES: FontEntry[] = [
  { file: 'LiberationSans-Regular', family: 'Liberation Sans', fontWeight: 400, fontStyle: 'normal' },
  { file: 'LiberationSans-Bold', family: 'Liberation Sans', fontWeight: 700, fontStyle: 'normal' },
  { file: 'LiberationSans-Italic', family: 'Liberation Sans', fontWeight: 400, fontStyle: 'italic' },
  { file: 'LiberationSans-BoldItalic', family: 'Liberation Sans', fontWeight: 700, fontStyle: 'italic' },
  { file: 'LiberationSerif-Regular', family: 'Liberation Serif', fontWeight: 400, fontStyle: 'normal' },
  { file: 'LiberationSerif-Bold', family: 'Liberation Serif', fontWeight: 700, fontStyle: 'normal' },
  { file: 'LiberationSerif-Italic', family: 'Liberation Serif', fontWeight: 400, fontStyle: 'italic' },
  { file: 'LiberationSerif-BoldItalic', family: 'Liberation Serif', fontWeight: 700, fontStyle: 'italic' },
  { file: 'LiberationMono-Regular', family: 'Liberation Mono', fontWeight: 400, fontStyle: 'normal' },
  { file: 'LiberationMono-Bold', family: 'Liberation Mono', fontWeight: 700, fontStyle: 'normal' },
  { file: 'LiberationMono-Italic', family: 'Liberation Mono', fontWeight: 400, fontStyle: 'italic' },
  { file: 'LiberationMono-BoldItalic', family: 'Liberation Mono', fontWeight: 700, fontStyle: 'italic' },
];

function decodeBase64(b64: string): Uint8Array {
  // Node Buffer where available (fast); atob fallback for WASM/browser.
  if (typeof Buffer !== 'undefined') {
    return new Uint8Array(Buffer.from(b64, 'base64'));
  }
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

/**
 * The 12 Liberation fonts as registrations, ready to pass to a Forme adapter's
 * `Font.register` (or a document's `fonts` array). Buffers are decoded fresh
 * per call so callers may transfer/detach them freely.
 */
export function standardFonts(): FontSpec[] {
  return ENTRIES.map((e) => ({
    family: e.family,
    src: decodeBase64(FONT_DATA_BASE64[e.file]),
    fontWeight: e.fontWeight,
    fontStyle: e.fontStyle,
  }));
}

/**
 * Base-14 family → Liberation family. The engine consults the same mapping in
 * pdfUa mode to route a standard font through the embedded Liberation program;
 * exported here for the CLI, docs, and JS-side pdfUa configuration. Style
 * (bold/italic) is carried by weight/style, so this maps family to family.
 * Symbol and ZapfDingbats have no metric-compatible substitute and are omitted
 * — a document using them in pdfUa mode is warned, not silently substituted.
 */
export const BASE14_ALIASES: Readonly<Record<string, string>> = Object.freeze({
  Helvetica: 'Liberation Sans',
  Arial: 'Liberation Sans',
  'Times-Roman': 'Liberation Serif',
  Times: 'Liberation Serif',
  'Times New Roman': 'Liberation Serif',
  Courier: 'Liberation Mono',
  'Courier New': 'Liberation Mono',
});
