import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockRenderDocument = vi.fn();

vi.mock('@formepdf/core', () => ({
  renderDocument: (...args: any[]) => mockRenderDocument(...args),
}));

vi.mock('../src/templates/index.js', () => {
  const invoiceFn = vi.fn().mockReturnValue({ type: 'Document', props: {} });
  return {
    getTemplate: (name: string) => (name === 'invoice' ? invoiceFn : null),
    listTemplates: () => [{ name: 'invoice' }],
  };
});

import { renderPdf, pdfResponse, pdfHandler } from '../src/index.js';

const PDF_BYTES = new Uint8Array([37, 80, 68, 70]);

describe('renderPdf', () => {
  beforeEach(() => {
    mockRenderDocument.mockReset();
    mockRenderDocument.mockResolvedValue(PDF_BYTES);
  });

  it('returns Uint8Array from render function', async () => {
    const renderFn = () => ({ type: 'Document', props: {} }) as any;
    const result = await renderPdf(renderFn);
    expect(result).toBe(PDF_BYTES);
  });

  it('renders from template name', async () => {
    const result = await renderPdf('invoice', { total: 100 });
    expect(result).toBe(PDF_BYTES);
  });

  it('throws for unknown template', async () => {
    await expect(renderPdf('nonexistent')).rejects.toThrow('Unknown template');
  });
});

describe('pdfResponse', () => {
  beforeEach(() => {
    mockRenderDocument.mockReset();
    mockRenderDocument.mockResolvedValue(PDF_BYTES);
  });

  it('returns Response with correct headers', async () => {
    const renderFn = () => ({ type: 'Document', props: {} }) as any;
    const response = await pdfResponse(renderFn);

    expect(response.headers.get('Content-Type')).toBe('application/pdf');
    expect(response.headers.get('Content-Disposition')).toContain('document.pdf');
  });

  it('uses template name as default filename', async () => {
    const response = await pdfResponse('invoice', {});
    expect(response.headers.get('Content-Disposition')).toContain('invoice.pdf');
  });
});

describe('pdfHandler', () => {
  beforeEach(() => {
    mockRenderDocument.mockReset();
    mockRenderDocument.mockResolvedValue(PDF_BYTES);
  });

  it('returns an async handler function (template mode)', async () => {
    const dataFn = async () => ({ total: 100 });
    const handler = pdfHandler('invoice', dataFn);

    expect(typeof handler).toBe('function');

    const req = new Request('http://localhost/pdf');
    const response = await handler(req, {});
    expect(response.headers.get('Content-Type')).toBe('application/pdf');
  });

  it('returns an async handler function (render function mode)', async () => {
    const renderFnFactory = async () => () =>
      ({ type: 'Document', props: {} }) as any;
    const handler = pdfHandler(renderFnFactory);

    const req = new Request('http://localhost/pdf');
    const response = await handler(req, {});
    expect(response.headers.get('Content-Type')).toBe('application/pdf');
  });

  it('returns 500 when dataFn throws', async () => {
    const dataFn = async () => {
      throw new Error('Data fetch failed');
    };
    const handler = pdfHandler('invoice', dataFn);

    const req = new Request('http://localhost/pdf');
    const response = await handler(req, {});
    expect(response.status).toBe(500);

    const body = await response.json();
    expect(body.error).toBe('PDF render failed');
  });
});
