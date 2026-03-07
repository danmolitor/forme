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

import { renderAndAttach } from '../src/render-attach.js';

const PDF_BYTES = new Uint8Array([37, 80, 68, 70]);

describe('renderAndAttach', () => {
  beforeEach(() => {
    mockRenderDocument.mockReset();
    mockRenderDocument.mockResolvedValue(PDF_BYTES);
  });

  it('renders from a function and returns attachment object', async () => {
    const render = () => ({ type: 'Document', props: {} }) as any;
    const result = await renderAndAttach({ render });

    expect(result.filename).toBe('document.pdf');
    expect(result.content).toBeInstanceOf(Buffer);
    expect(result.content.length).toBe(PDF_BYTES.length);
  });

  it('renders from a template name', async () => {
    const result = await renderAndAttach({ template: 'invoice', data: { total: 100 } });

    expect(result.filename).toBe('invoice.pdf');
    expect(result.content).toBeInstanceOf(Buffer);
  });

  it('uses custom filename', async () => {
    const render = () => ({ type: 'Document', props: {} }) as any;
    const result = await renderAndAttach({ render, filename: 'custom.pdf' });

    expect(result.filename).toBe('custom.pdf');
  });

  it('throws when neither template nor render is provided', async () => {
    await expect(renderAndAttach({})).rejects.toThrow(
      'Either "template" or "render" must be provided'
    );
  });

  it('throws for unknown template', async () => {
    await expect(renderAndAttach({ template: 'nonexistent' })).rejects.toThrow(
      'Unknown template'
    );
  });
});
