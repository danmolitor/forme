import type { FormeFont } from './types.js';

/// Options for registering a custom font.
export interface FontRegistration {
  family: string;
  src: string | Uint8Array;
  fontWeight?: number | 'normal' | 'bold';
  fontStyle?: 'normal' | 'italic' | 'oblique';
}

const globalFonts: FontRegistration[] = [];

function normalizeWeight(w?: number | string): number {
  if (w === undefined || w === 'normal') return 400;
  if (w === 'bold') return 700;
  return typeof w === 'number' ? w : (parseInt(w, 10) || 400);
}

export const Font = {
  register(options: FontRegistration): void {
    globalFonts.push({
      ...options,
      fontWeight: normalizeWeight(options.fontWeight),
      fontStyle: options.fontStyle || 'normal',
    });
  },

  clear(): void {
    globalFonts.length = 0;
  },

  getRegistered(): FontRegistration[] {
    return [...globalFonts];
  },
};

// ─── Font merging ─────────────────────────────────────────────────

function normalizeFontWeight(w?: number | string): number {
  if (w === undefined || w === 'normal') return 400;
  if (w === 'bold') return 700;
  return typeof w === 'number' ? w : (parseInt(w, 10) || 400);
}

function fontKey(family: string, weight: number, italic: boolean): string {
  return `${family}:${weight}:${italic}`;
}

export function mergeFonts(
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
