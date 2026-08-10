import type { Style } from '@formepdf/shared';

export const StyleSheet = {
  create<T extends Record<string, Style>>(styles: T): T {
    return styles;
  },
};
