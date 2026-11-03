import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { getDocument } from 'pdfjs-dist/legacy/build/pdf.mjs';
import { expect, it } from 'vitest';
import { renderPdf } from '../src/index';

const popplerAvailable = spawnSync('pdftotext', ['-v']).status === 0;

const cases = [
  ['NotoSansHebrew', 'שָׁלוֹם עוֹלָם', 'rtl'],
  ['NotoSansHebrew', '123\nשָׁלוֹם עוֹלָם', 'rtl'],
  ['NotoNaskhArabic', 'مَرْحَبًا بِالْعَالَم', 'rtl'],
  // A variation selector in a cluster (#176): PDF.js moved the base letter
  // to the end of the line.
  ['NotoSansHebrew', 'שָׁ️לוֹם עוֹלָם', 'rtl'],
  ['NotoSansDevanagari', 'नमस्ते दुनिया अनुभव कौशल', 'ltr'],
  ['NotoSansDevanagari', 'कु कू के कै को कौ कि की कं कौ के कु कू', 'ltr'],
  ['NotoSansDevanagari', 'र्कि र्की र्को र्कौ क्षि त्रि', 'ltr'],
] as const;

it.each(cases)('extracts %s: %s', async (family, text, direction) => {
  const font = readFileSync(new URL(`../../../engine/tests/fixtures/fonts/${family}-Regular.ttf`, import.meta.url));
  for (const runs of [false, true]) {
    const pdf = await renderPdf(JSON.stringify({
      fonts: [{ family, weight: 400, src: font.toString('base64') }],
      defaultStyle: { fontFamily: family, fontSize: 14, direction },
      children: [{
        kind: runs ? { type: 'Text', content: '', runs: [{ content: text }] } : { type: 'Text', content: text },
        style: {}, children: [],
      }],
    }));
    const task = getDocument({ data: new Uint8Array(pdf) });
    try {
      const doc = await task.promise;
      const page = await doc.getPage(1);
      const content = await page.getTextContent();
      const extracted = content.items.map(item => 'str' in item ? item.str : '').join('');
      expect(extracted, runs ? 'styled text' : 'plain text').toBe(text.replaceAll('\n', ''));
      if (popplerAvailable) {
        const dir = mkdtempSync(join(tmpdir(), 'forme-unicode-'));
        try {
          const path = join(dir, 'text.pdf');
          writeFileSync(path, pdf);
          const poppler = execFileSync('pdftotext', [path, '-'], { encoding: 'utf8' });
          // Poppler adds directional wrappers and page/line separators.
          expect(poppler.replace(/[\u202A-\u202E\n\r\f]/g, '').trim(), 'pdftotext')
            .toBe(text.replaceAll('\n', ''));
        } finally {
          rmSync(dir, { recursive: true, force: true });
        }
      }
    } finally {
      await task.destroy();
    }
  }
}, 30_000);

// A combining grapheme joiner in a cluster (#176). Like a variation
// selector it draws nothing; the shaper used to hide it as the font's space
// glyph, whose width PDF.js ignores for a nonspacing mark, and PDF.js split
// the line there and moved the base letter to the end. Poppler extracts the
// text exactly; PDF.js keeps the base letter in place but orders the two
// invisible marks after it its own way, so the CGJ is set aside there.
it('keeps the base letter in place around a combining grapheme joiner', async () => {
  const family = 'NotoNaskhArabic';
  const text = 'مَ\u034Fرْحَبًا بِالْعَالَم';
  const font = readFileSync(new URL(`../../../engine/tests/fixtures/fonts/${family}-Regular.ttf`, import.meta.url));
  const pdf = await renderPdf(JSON.stringify({
    fonts: [{ family, weight: 400, src: font.toString('base64') }],
    defaultStyle: { fontFamily: family, fontSize: 14, direction: 'rtl' },
    children: [{ kind: { type: 'Text', content: text }, style: {}, children: [] }],
  }));
  const task = getDocument({ data: new Uint8Array(pdf) });
  try {
    const content = await (await (await task.promise).getPage(1)).getTextContent();
    const extracted = content.items.map(item => 'str' in item ? item.str : '').join('');
    expect(extracted.replace('\u034F', '')).toBe(text.replace('\u034F', ''));
  } finally {
    await task.destroy();
  }
  if (popplerAvailable) {
    const dir = mkdtempSync(join(tmpdir(), 'forme-unicode-'));
    try {
      const path = join(dir, 'text.pdf');
      writeFileSync(path, pdf);
      const poppler = execFileSync('pdftotext', [path, '-'], { encoding: 'utf8' });
      expect(poppler.replace(/[\u202A-\u202E\n\r\f]/g, '').trim()).toBe(text);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }
}, 30_000);
