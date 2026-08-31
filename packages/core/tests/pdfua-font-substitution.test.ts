import { describe, it, expect } from 'vitest';
import React from 'react';
import { serialize, Document, Page, Text } from '@formepdf/react';
import { standardFonts } from '@formepdf/fonts-standard';
import { renderPdfWithLayout } from '../src/index';

const h = React.createElement;

function docWith(text: string, pdfUa: boolean) {
  const doc = serialize(
    h(Document, null, h(Page, { size: 'Letter', margin: 48 }, h(Text, { style: { fontSize: 12 } }, text))),
  ) as Record<string, unknown>;
  if (pdfUa) {
    doc.pdfUa = true;
    doc.tagged = true;
    doc.metadata = { lang: 'en-US' };
    doc.fonts = standardFonts().map((f) => ({
      family: f.family,
      src: Buffer.from(f.src).toString('base64'),
      weight: f.fontWeight,
      italic: f.fontStyle === 'italic',
    }));
  }
  return JSON.stringify(doc);
}

/** Parse the /Widths array of the embedded simple TrueType font. Index 0 = WinAnsi code 32. */
function trueTypeWidths(pdf: Uint8Array): number[] {
  const s = Buffer.from(pdf).toString('latin1');
  const m = s.match(/\/Subtype\s*\/TrueType[\s\S]*?\/Widths\s*\[([^\]]*)\]/);
  if (!m) throw new Error('no embedded TrueType /Widths found');
  return m[1].trim().split(/\s+/).map(Number);
}

describe('pdfUa font substitution (Liberation embedding + PDF/A width carve-out)', () => {
  it('embeds a TrueType program in pdfUa mode; base-14 stays non-embedded by default', async () => {
    const { pdf: def } = await renderPdfWithLayout(docWith('Hello World', false));
    const { pdf: ua } = await renderPdfWithLayout(docWith('Hello World', true));

    const defStr = Buffer.from(def).toString('latin1');
    const uaStr = Buffer.from(ua).toString('latin1');

    // Default: non-embedded standard font, no FontFile2.
    expect(defStr).toContain('/Subtype /Type1');
    expect(defStr).not.toContain('/FontFile2');

    // pdfUa: embedded simple TrueType.
    expect(uaStr).toContain('/Subtype /TrueType');
    expect(uaStr).toContain('/FontFile2');
    expect(uaStr).toContain('/Encoding /WinAnsiEncoding');
  });

  it('declares AFM widths for common glyphs (exact positioning preserved)', async () => {
    const { pdf } = await renderPdfWithLayout(docWith('AVWy10', true));
    const w = trueTypeWidths(pdf);
    // Helvetica AFM: 'A'(65)=667, ' '(32)=278, '0'(48)=556 — index = code - 32.
    expect(w[65 - 32]).toBe(667);
    expect(w[32 - 32]).toBe(278);
    expect(w[48 - 32]).toBe(556);
  });

  it('carves out the divergent glyphs to the substitute advance (PDF/A consistency)', async () => {
    const { pdf } = await renderPdfWithLayout(docWith('¯ ` · ÷ ±', true));
    const w = trueTypeWidths(pdf);
    // These base-14 AFM widths disagree with Liberation's actual advances, so
    // the declared width must NOT be the AFM value (it's the program's advance).
    const AFM: Record<number, number> = {
      175: 333, // macron ¯
      96: 222, // grave `
      167: 278, // middot ·
      247: 584, // divide ÷
      177: 584, // plusminus ±
    };
    for (const [codeStr, afm] of Object.entries(AFM)) {
      const code = Number(codeStr);
      expect(w[code - 32], `code ${code} should be carved off AFM ${afm}`).not.toBe(afm);
    }
  });
});
