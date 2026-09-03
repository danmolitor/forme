import { describe, it, expect } from 'vitest';
import { applyAttachmentOptions } from '../src/attachments';

describe('applyAttachmentOptions (Factur-X container mapping)', () => {
  it('maps facturX to a spec-named attachment + zugferd XMP meta', () => {
    const doc: Record<string, unknown> = {};
    applyAttachmentOptions(doc, {
      facturX: { xml: '<xml/>', profile: 'EN 16931' },
    });
    const atts = doc.attachments as Array<Record<string, unknown>>;
    expect(atts).toHaveLength(1);
    expect(atts[0].name).toBe('factur-x.xml');
    expect(atts[0].mimeType).toBe('text/xml');
    expect(atts[0].src).toBe(Buffer.from('<xml/>').toString('base64'));
    expect(doc.zugferd).toMatchObject({
      conformanceLevel: 'EN 16931',
      documentFileName: 'factur-x.xml',
    });
  });

  it('XRECHNUNG profile defaults the filename to xrechnung.xml', () => {
    const doc: Record<string, unknown> = {};
    applyAttachmentOptions(doc, {
      facturX: { xml: new Uint8Array([60, 47, 62]), profile: 'XRECHNUNG' },
    });
    const atts = doc.attachments as Array<Record<string, unknown>>;
    expect(atts[0].name).toBe('xrechnung.xml');
    expect((doc.zugferd as Record<string, unknown>).documentFileName).toBe('xrechnung.xml');
  });

  it('plain attachments pass through without zugferd meta', () => {
    const doc: Record<string, unknown> = {};
    applyAttachmentOptions(doc, {
      attachments: [{ name: 'report.csv', data: 'a,b\n1,2', mimeType: 'text/csv' }],
    });
    expect(doc.zugferd).toBeUndefined();
    const atts = doc.attachments as Array<Record<string, unknown>>;
    expect(atts[0].name).toBe('report.csv');
  });

  it('no options leaves the doc untouched', () => {
    const doc: Record<string, unknown> = {};
    applyAttachmentOptions(doc, {});
    expect(doc.attachments).toBeUndefined();
    expect(doc.zugferd).toBeUndefined();
  });
});
