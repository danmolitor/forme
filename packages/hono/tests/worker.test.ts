import { describe, it, expect, vi } from 'vitest';
import { readFileSync } from 'node:fs';

// The Workers entry (./src/worker.ts) must initialise @formepdf/core's worker
// build with the WebAssembly.Module that Wrangler hands it, and render through
// it. Before it existed, Workers fell back to the browser build, which
// Wrangler cannot instantiate ("__wbindgen_start is not a function").

// vi.mock factories are hoisted above the imports, so their values are too.
const { wasmModule, mockInit, mockRenderDocument, mockBrowserRender } = vi.hoisted(() => ({
  wasmModule: { kind: 'compiled WebAssembly.Module' },
  mockInit: vi.fn().mockResolvedValue(undefined),
  mockRenderDocument: vi.fn().mockResolvedValue(new Uint8Array([37, 80, 68, 70])),
  mockBrowserRender: vi.fn(),
}));

vi.mock('@formepdf/core/worker', () => ({
  init: (...args: any[]) => mockInit(...args),
  renderDocument: (...args: any[]) => mockRenderDocument(...args),
}));
vi.mock('@formepdf/core/pkg-web/forme_bg.wasm', () => ({ default: wasmModule }));
vi.mock('@formepdf/core/browser', () => ({ renderDocument: mockBrowserRender }));
vi.mock('@formepdf/templates', () => ({
  getTemplate: () => null,
  listTemplates: () => [],
}));

import { pdfResponse } from '../src/worker.js';

describe('Workers entry', () => {
  it('initialises the worker build with the imported module and renders through it', async () => {
    const response = await pdfResponse(() => ({ type: 'Document', props: {} }) as any);

    expect(mockInit).toHaveBeenCalledWith(wasmModule);
    expect(mockRenderDocument).toHaveBeenCalledTimes(1);
    expect(mockBrowserRender).not.toHaveBeenCalled();
    expect(response.headers.get('Content-Type')).toBe('application/pdf');
  });

  it('is what the "worker" export condition resolves to', () => {
    const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'));
    expect(pkg.exports['.'].worker.import).toBe('./dist/worker.js');
    expect(pkg.exports['.'].import).toBe('./dist/index.js');
  });
});
