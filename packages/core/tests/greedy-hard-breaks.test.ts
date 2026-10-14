import { getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs';
import { expect, it } from 'vitest';
import { renderPdfWithLayout } from '../src/index';

it.each([false, true])('does not draw trailing hard breaks in greedy text (runs=%s)', async (runs) => {
  for (const ending of ['\n', '\r', '\r\n', '\u2028', '\u2029']) {
    const result = await renderPdfWithLayout(JSON.stringify({
      defaultStyle: { fontFamily: 'Helvetica', fontSize: 12, lineBreaking: 'greedy' },
      children: [{
        kind: runs
          ? { type: 'Text', content: '', runs: [
            { content: 'Hello', style: { fontWeight: 700 } },
            { content: ending },
          ] }
          : { type: 'Text', content: `Hello${ending}` },
        style: {}, children: [],
      }],
    }));
    const task = getDocument({ data: new Uint8Array(result.pdf), useSystemFonts: true });
    try {
      const pdf = await task.promise;
      const page = await pdf.getPage(1);
      const content = await page.getTextContent();
      const extracted = content.items.map(item => 'str' in item ? item.str : '').join('');
      expect(extracted, JSON.stringify(ending)).toBe('Hello');
      expect(result.warnings, JSON.stringify(ending)).toEqual([]);
    } finally {
      await task.destroy();
    }
  }
});
