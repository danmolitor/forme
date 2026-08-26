import type { Style } from './types.js';

export const StyleSheet = {
  create<T extends Record<string, Style>>(styles: T): T {
    return styles;
  },
};
