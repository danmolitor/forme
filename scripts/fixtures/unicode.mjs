import { readFileSync } from 'node:fs';

// Shaped scripts exercise CID aliases, nonzero cluster advances and surplus
// glyphs. The old outline approach passed the Latin corpus but failed widths.
export function unicodeDocument() {
  const cases = [
    ['NotoSansDevanagari', 'नमस्ते दुनिया अनुभव कौशल\nकु कू के कै को कौ कि की कं कौ के कु कू', 'ltr'],
    ['NotoSansHebrew', '123\nשָׁלוֹם עוֹלָם', 'rtl'],
    ['NotoNaskhArabic', 'مَرْحَبًا بِالْعَالَم', 'rtl'],
  ];
  return {
    metadata: { title: 'Shaped Unicode text', lang: 'en-US' },
    fonts: cases.map(([family]) => ({
      family, weight: 400,
      src: readFileSync(new URL(`../../engine/tests/fixtures/fonts/${family}-Regular.ttf`, import.meta.url)).toString('base64'),
    })),
    children: cases.map(([family, content, direction]) => ({
      kind: { type: 'Text', content },
      style: { fontFamily: family, fontSize: 14, direction },
      children: [],
    })),
  };
}
