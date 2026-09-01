import { describe, it, expect } from 'vitest';
import React from 'react';
import { serialize, Document, Page, Text } from '@formepdf/react';
import { standardFonts } from '@formepdf/fonts-standard';
import { renderPdfWithLayout } from '../src/index';

const h = React.createElement;

/** A doc requesting BOTH PDF/A-2b and PDF/UA-1, with fonts-standard registered
 *  under its own Liberation family names (exactly as `standardFonts()` yields). */
function composeDoc() {
  const doc = serialize(
    h(Document, null, h(Page, { size: 'Letter', margin: 48 },
      h(Text, { style: { fontSize: 12 } }, 'Archival and accessible at once.'))),
  ) as Record<string, unknown>;
  doc.pdfa = '2b';
  doc.pdfUa = true;
  doc.tagged = true;
  doc.metadata = { lang: 'en-US', title: 'Compose' };
  doc.fonts = standardFonts().map((f) => ({
    family: f.family, src: Buffer.from(f.src).toString('base64'),
    weight: f.fontWeight, italic: f.fontStyle === 'italic',
  }));
  return JSON.stringify(doc);
}

describe('PDF/A composes with PDF/UA', () => {
  it('renders pdfA + pdfUa with base-14 substituted via fonts-standard (used to throw)', async () => {
    const { pdf, warnings } = await renderPdfWithLayout(composeDoc());
    const s = Buffer.from(pdf).toString('latin1');

    // The base-14 PDF/A embed check must accept the pdfUa Liberation
    // substitution — the render succeeds instead of throwing "all fonts must
    // be embedded", and no font warning is raised.
    expect(String.fromCharCode(...pdf.slice(0, 5))).toBe('%PDF-');
    expect(warnings.some((w) => w.startsWith('pdfUa:') && /not embedded/.test(w))).toBe(false);
    // Fonts are actually embedded (simple TrueType program), not a bare base-14 Type1.
    expect(s).toContain('/FontFile2');
    // Both regimes are active in the output.
    expect(s).toContain('/StructTreeRoot'); // pdfUa/tagged
    expect(s).toContain('/OutputIntent'); // pdfA
  });
});
