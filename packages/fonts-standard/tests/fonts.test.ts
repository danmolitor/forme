import { describe, it, expect } from 'vitest';
import { standardFonts, BASE14_ALIASES, type FontSpec } from '../src/index.js';

describe('@formepdf/fonts-standard', () => {
  const fonts = standardFonts();

  it('returns all 12 Liberation fonts (3 families × 4 styles)', () => {
    expect(fonts).toHaveLength(12);
    const families = new Set(fonts.map((f) => f.family));
    expect([...families].sort()).toEqual(['Liberation Mono', 'Liberation Sans', 'Liberation Serif']);
    for (const family of families) {
      const styles = fonts
        .filter((f) => f.family === family)
        .map((f) => `${f.fontWeight}/${f.fontStyle}`)
        .sort();
      expect(styles).toEqual(['400/italic', '400/normal', '700/italic', '700/normal']);
    }
  });

  it('every buffer is a valid non-empty TrueType program', () => {
    for (const f of fonts) {
      expect(f.src).toBeInstanceOf(Uint8Array);
      expect(f.src.length).toBeGreaterThan(10000);
      // TrueType sfnt version: 0x00010000 (or 'true'/'OTTO'). Liberation is TTF.
      const tag = f.src.subarray(0, 4);
      expect([tag[0], tag[1], tag[2], tag[3]]).toEqual([0x00, 0x01, 0x00, 0x00]);
    }
  });

  it('decodes fresh buffers per call (callers may transfer them)', () => {
    const a = standardFonts()[0].src;
    const b = standardFonts()[0].src;
    expect(a).not.toBe(b);
    expect(a).toEqual(b);
  });

  it('maps the base-14 families to Liberation equivalents', () => {
    expect(BASE14_ALIASES.Helvetica).toBe('Liberation Sans');
    expect(BASE14_ALIASES['Times-Roman']).toBe('Liberation Serif');
    expect(BASE14_ALIASES.Courier).toBe('Liberation Mono');
    // every alias target is a family this package actually provides
    const provided = new Set(fonts.map((f: FontSpec) => f.family));
    for (const target of Object.values(BASE14_ALIASES)) {
      expect(provided.has(target)).toBe(true);
    }
  });
});
