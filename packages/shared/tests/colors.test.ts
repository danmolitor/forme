import { describe, expect, it } from 'vitest';
import { mapStyle, parseColor } from '../src/index.js';

describe('parseColor', () => {
  it('reads CSS colour names, case-insensitively', () => {
    expect(parseColor('red')).toEqual({ r: 1, g: 0, b: 0, a: 1 });
    expect(parseColor('RebeccaPurple')).toEqual({ r: 102 / 255, g: 51 / 255, b: 153 / 255, a: 1 });
    expect(parseColor('transparent')).toEqual({ r: 0, g: 0, b: 0, a: 0 });
  });

  it('never produces NaN channels', () => {
    // "red" used to be read as three-digit hex: r was NaN, serialized as
    // null, and the engine rejected the whole document (#234).
    for (const v of ['notacolor', '#zzz', '#12', 'bleu']) {
      const c = parseColor(v);
      expect([c.r, c.g, c.b, c.a].every(Number.isFinite), v).toBe(true);
    }
  });

  it('a named colour survives JSON serialization', () => {
    const style = mapStyle({ backgroundColor: 'red', borderColor: 'navy', color: 'white' });
    expect(JSON.stringify(style)).not.toContain('null');
  });
});
