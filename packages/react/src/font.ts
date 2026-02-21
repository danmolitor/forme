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
