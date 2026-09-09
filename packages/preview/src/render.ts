// The parity core. Kept free of React so it is unit-testable in Node and so the
// byte-identity guarantee is expressed as a plain function, not a component.

import { standardFonts as liberationFonts } from '@formepdf/fonts-standard';
import type { RenderHtmlOptions, RenderHtmlResult } from '@formepdf/html';

/**
 * A font expressed the way `@formepdf/html` wants it: bytes (or base64), never a
 * path or URL — the browser HTML render path cannot fetch, so bytes are the one
 * form that renders identically on the server and in the preview.
 */
export interface PreviewFont {
  family: string;
  /** Raw TTF bytes, or a base64 string. */
  data: Uint8Array | string;
  /** CSS weight (400 regular, 700 bold). Default 400. */
  weight?: number;
  italic?: boolean;
}

/** The shape of `renderHtml` from any `@formepdf/html` entry (node/browser/worker). */
export type RenderHtmlFn = (
  html: string,
  options?: RenderHtmlOptions,
) => RenderHtmlResult | Promise<RenderHtmlResult>;

/**
 * Render a document for preview.
 *
 * This is deliberately a faithful passthrough: it calls the injected
 * `renderHtml` with the caller's EXACT `(html, options)`. There is no
 * preview-specific option, default, or transform — that absence is the parity
 * guarantee. Because the browser component passes the browser `renderHtml` and
 * the server passes the node one, and this function changes nothing between
 * them, the preview bytes equal the server bytes. `tests/parity.test.ts` locks
 * exactly that; if a future change makes this anything more than a passthrough,
 * the test goes red.
 *
 * `renderHtml` is injected rather than imported so this module carries no WASM
 * and tests can drive it with the Node/worker entry.
 */
export async function renderForPreview(
  html: string,
  options: RenderHtmlOptions | undefined,
  renderHtml: RenderHtmlFn,
): Promise<RenderHtmlResult> {
  return await renderHtml(html, options);
}

/**
 * `@formepdf/fonts-standard` adapted to `renderHtml`'s font shape.
 *
 * fonts-standard emits `{ family, src, fontWeight, fontStyle }`; `renderHtml`
 * wants `{ family, data, weight, italic }`. Hand-mapping this on each side is
 * exactly where preview and server silently drift, so this is the one blessed
 * adapter — pass its output to BOTH your server `renderHtml` and `<FormePreview
 * options>`.
 */
export function standardFonts(): PreviewFont[] {
  return liberationFonts().map((f) => ({
    family: f.family,
    data: f.src,
    weight: f.fontWeight,
    italic: f.fontStyle === 'italic',
  }));
}

/**
 * A stable, order-independent, content-sensitive fingerprint of a font set.
 *
 * The point is to make the silent failure loud: a template referencing a font
 * that the server embeds but the preview doesn't renders one way on each side
 * with no error. Fingerprint the fonts you hand each side; if the prints
 * differ, the preview cannot be trusted to match — surface it. Equal set,
 * any order → equal print; a different family/weight/style OR different bytes →
 * different print. Undefined and empty are the same, stable value.
 */
export function fontFingerprint(fonts?: PreviewFont[]): string {
  if (!fonts || fonts.length === 0) return 'fonts:0';
  const lines = fonts
    .map((f) => `${f.family}|${f.weight ?? 400}|${f.italic ? 'i' : 'n'}|${hash(toBytes(f.data))}`)
    .sort();
  return `fonts:${fonts.length}:${hash(toBytes(lines.join('\n')))}`;
}

/**
 * Do these fonts match what the other side was given? A convenience over
 * `fontFingerprint` for the component's mismatch banner. `undefined` expected
 * print means "no expectation" → always matches.
 */
export function fontsMatch(fonts: PreviewFont[] | undefined, expectedFingerprint?: string): boolean {
  if (expectedFingerprint == null) return true;
  return fontFingerprint(fonts) === expectedFingerprint;
}

// ── portable hashing (no crypto dependency; runs in Node and the browser) ────

function toBytes(data: Uint8Array | string): Uint8Array {
  return typeof data === 'string' ? new TextEncoder().encode(data) : data;
}

/** FNV-1a in two 32-bit lanes → 16 hex chars. Enough to fingerprint font sets. */
function hash(bytes: Uint8Array): string {
  let h1 = 0x811c9dc5;
  let h2 = 0xc9dc5118;
  for (let i = 0; i < bytes.length; i++) {
    const b = bytes[i];
    h1 = Math.imul(h1 ^ b, 0x01000193);
    h2 = Math.imul(h2 ^ b, 0x85ebca77);
  }
  return (h1 >>> 0).toString(16).padStart(8, '0') + (h2 >>> 0).toString(16).padStart(8, '0');
}
