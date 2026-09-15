import { describe, it, expect, vi, beforeEach } from 'vitest';

/** The engine's warnings are the render-defect channel: what the renderer did
 *  that you didn't ask for. `renderDocToResult` dropped them for as long as a
 *  comment claiming the core binding surfaced none outlived its truth, so
 *  `forme build`, `forme dev`, and the VS Code preview — every JSX/SFC render
 *  — went silent on sequential flex-row splits, missing glyphs, and clamped
 *  table columns. A user hit exactly that: two columns serialized across a
 *  page break, warnings reported as "none", issue filed.
 *
 *  Core is mocked because this is a plumbing contract, not a layout test: the
 *  question is only whether what the binding returns reaches the caller. */
const renderPdfWithLayout = vi.fn();
vi.mock('@formepdf/core', () => ({
  renderPdfWithLayout: (...args: unknown[]) => renderPdfWithLayout(...args),
}));
vi.mock('../src/resolve.js', () => ({
  resolveAllSources: vi.fn().mockResolvedValue(undefined),
}));

const { renderDocToResult } = await import('../src/render.js');

const DEFECT =
  'render defect: a flex row crossing a page boundary lays its children sequentially';

describe('renderDocToResult warnings', () => {
  beforeEach(() => renderPdfWithLayout.mockReset());

  it('passes engine warnings through to the caller', async () => {
    renderPdfWithLayout.mockResolvedValue({
      pdf: new Uint8Array([1]),
      layout: { pages: [] },
      warnings: [DEFECT],
    });
    const out = await renderDocToResult({ children: [] }, { startTime: 0 });
    expect(out.warnings).toEqual([DEFECT]);
  });

  it('reports no warnings as an empty array, never undefined', async () => {
    renderPdfWithLayout.mockResolvedValue({
      pdf: new Uint8Array([1]),
      layout: { pages: [] },
      warnings: [],
    });
    const out = await renderDocToResult({ children: [] }, { startTime: 0 });
    expect(out.warnings).toEqual([]);
  });

  it('tolerates a binding that omits the field', async () => {
    renderPdfWithLayout.mockResolvedValue({ pdf: new Uint8Array([1]), layout: { pages: [] } });
    const out = await renderDocToResult({ children: [] }, { startTime: 0 });
    expect(out.warnings).toEqual([]);
  });
});
