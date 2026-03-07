import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockRenderFromFile = vi.fn();
const mockWriteFile = vi.fn();

vi.mock('@formepdf/renderer', () => ({
  renderFromFile: (...args: any[]) => mockRenderFromFile(...args),
}));

vi.mock('node:fs/promises', () => ({
  writeFile: (...args: any[]) => mockWriteFile(...args),
}));

import { buildPdf } from '../src/build.js';

describe('buildPdf', () => {
  beforeEach(() => {
    mockRenderFromFile.mockReset();
    mockWriteFile.mockReset();
    vi.spyOn(console, 'log').mockImplementation(() => {});
  });

  it('calls renderFromFile with the resolved input path', async () => {
    const pdfBytes = new Uint8Array([37, 80, 68, 70]);
    mockRenderFromFile.mockResolvedValue({ pdf: pdfBytes });
    mockWriteFile.mockResolvedValue(undefined);

    await buildPdf('template.tsx', { output: 'out.pdf' });

    expect(mockRenderFromFile).toHaveBeenCalledTimes(1);
    const [inputPath, opts] = mockRenderFromFile.mock.calls[0];
    expect(inputPath).toContain('template.tsx');
    expect(opts).toEqual({ dataPath: undefined });
  });

  it('passes dataPath option through', async () => {
    mockRenderFromFile.mockResolvedValue({ pdf: new Uint8Array(4) });
    mockWriteFile.mockResolvedValue(undefined);

    await buildPdf('template.tsx', { output: 'out.pdf', dataPath: 'data.json' });

    const [, opts] = mockRenderFromFile.mock.calls[0];
    expect(opts).toEqual({ dataPath: 'data.json' });
  });

  it('writes PDF bytes to the output path', async () => {
    const pdfBytes = new Uint8Array([37, 80, 68, 70]);
    mockRenderFromFile.mockResolvedValue({ pdf: pdfBytes });
    mockWriteFile.mockResolvedValue(undefined);

    await buildPdf('template.tsx', { output: 'out.pdf' });

    expect(mockWriteFile).toHaveBeenCalledTimes(1);
    const [outputPath, data] = mockWriteFile.mock.calls[0];
    expect(outputPath).toContain('out.pdf');
    expect(data).toBe(pdfBytes);
  });
});
