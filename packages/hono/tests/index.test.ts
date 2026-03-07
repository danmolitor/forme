import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockRenderDocument = vi.fn();

vi.mock('@formepdf/core', () => ({
  renderDocument: (...args: any[]) => mockRenderDocument(...args),
}));

// Mock the templates module
vi.mock('../src/templates/index.js', () => {
  const invoiceFn = vi.fn().mockReturnValue({ type: 'Document', props: {} });
  return {
    getTemplate: (name: string) => (name === 'invoice' ? invoiceFn : null),
    listTemplates: () => [{ name: 'invoice' }, { name: 'receipt' }],
  };
});

import { pdfResponse, formePdf, listTemplates } from '../src/index.js';

const PDF_BYTES = new Uint8Array([37, 80, 68, 70]); // %PDF

describe('pdfResponse', () => {
  beforeEach(() => {
    mockRenderDocument.mockReset();
    mockRenderDocument.mockResolvedValue(PDF_BYTES);
  });

  it('renders from a function and returns Response with PDF headers', async () => {
    const renderFn = () => ({ type: 'Document', props: {} }) as any;
    const response = await pdfResponse(renderFn);

    expect(mockRenderDocument).toHaveBeenCalled();
    expect(response.headers.get('Content-Type')).toBe('application/pdf');
    expect(response.headers.get('Content-Disposition')).toContain('document.pdf');
  });

  it('renders from a template name', async () => {
    const response = await pdfResponse('invoice', { total: 100 });

    expect(mockRenderDocument).toHaveBeenCalled();
    expect(response.headers.get('Content-Disposition')).toContain('invoice.pdf');
  });

  it('returns 400 for unknown template', async () => {
    const response = await pdfResponse('nonexistent', {});
    expect(response.status).toBe(400);
  });

  it('respects custom filename', async () => {
    const renderFn = () => ({ type: 'Document', props: {} }) as any;
    const response = await pdfResponse(renderFn, { filename: 'custom.pdf' });

    expect(response.headers.get('Content-Disposition')).toContain('custom.pdf');
  });

  it('sets attachment disposition when download is true', async () => {
    const renderFn = () => ({ type: 'Document', props: {} }) as any;
    const response = await pdfResponse(renderFn, { download: true });

    expect(response.headers.get('Content-Disposition')).toMatch(/^attachment/);
  });
});

describe('listTemplates', () => {
  it('returns template entries', () => {
    const templates = listTemplates();
    expect(templates).toEqual([{ name: 'invoice' }, { name: 'receipt' }]);
  });
});

describe('formePdf middleware', () => {
  it('adds pdf method to context', async () => {
    const middleware = formePdf();
    const context: any = {};
    const next = vi.fn().mockResolvedValue(undefined);

    await middleware(context, next);

    expect(typeof context.pdf).toBe('function');
    expect(next).toHaveBeenCalled();
  });
});
