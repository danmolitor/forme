import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

vi.mock('node:fs/promises', () => ({
  readFile: vi.fn(),
}));

import { readFile } from 'node:fs/promises';
import {
  resolveFontSources,
  resolveImageSources,
  uint8ArrayToBase64,
} from '../src/resolve.js';

const mockReadFile = vi.mocked(readFile);

describe('uint8ArrayToBase64', () => {
  it('converts bytes to base64', () => {
    const bytes = new Uint8Array([72, 101, 108, 108, 111]); // "Hello"
    expect(uint8ArrayToBase64(bytes)).toBe('SGVsbG8=');
  });
});

describe('resolveFontSources', () => {
  beforeEach(() => {
    mockReadFile.mockReset();
  });

  it('does nothing when no fonts are present', async () => {
    const doc = { children: [] };
    await resolveFontSources(doc);
    expect(mockReadFile).not.toHaveBeenCalled();
  });

  it('converts Uint8Array font sources to base64', async () => {
    const bytes = new Uint8Array([1, 2, 3, 4]);
    const doc = { fonts: [{ family: 'Test', src: bytes }] };
    await resolveFontSources(doc as any);
    expect(doc.fonts[0].src).toBe(uint8ArrayToBase64(bytes));
  });

  it('reads file paths and converts to base64', async () => {
    const fileBytes = Buffer.from([0x00, 0x01, 0x02]);
    mockReadFile.mockResolvedValue(fileBytes);

    const doc = { fonts: [{ family: 'Test', src: 'fonts/test.ttf' }] };
    await resolveFontSources(doc as any, '/project');

    expect(mockReadFile).toHaveBeenCalledWith('/project/fonts/test.ttf');
    expect(doc.fonts[0].src).toBe(uint8ArrayToBase64(new Uint8Array(fileBytes)));
  });

  it('passes data URIs through unchanged', async () => {
    const dataUri = 'data:font/ttf;base64,AAEC';
    const doc = { fonts: [{ family: 'Test', src: dataUri }] };
    await resolveFontSources(doc as any);

    expect(doc.fonts[0].src).toBe(dataUri);
    expect(mockReadFile).not.toHaveBeenCalled();
  });
});

describe('resolveImageSources', () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    globalThis.fetch = vi.fn();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
  });

  it('does nothing when no children', async () => {
    const doc = {};
    await resolveImageSources(doc);
  });

  it('converts HTTP image URLs to base64 data URIs', async () => {
    const mockFetch = vi.mocked(globalThis.fetch);
    const imageBytes = new Uint8Array([0x89, 0x50, 0x4e, 0x47]);
    mockFetch.mockResolvedValue({
      ok: true,
      arrayBuffer: () => Promise.resolve(imageBytes.buffer),
      headers: new Headers({ 'content-type': 'image/png' }),
    } as Response);

    const doc = {
      children: [
        { kind: { type: 'Image', src: 'https://example.com/logo.png' } },
      ],
    };
    await resolveImageSources(doc);

    expect(mockFetch).toHaveBeenCalledWith('https://example.com/logo.png');
    expect((doc.children[0].kind as any).src).toMatch(/^data:image\/png;base64,/);
  });

  it('throws on failed fetch', async () => {
    const mockFetch = vi.mocked(globalThis.fetch);
    mockFetch.mockResolvedValue({
      ok: false,
      status: 404,
      headers: new Headers(),
    } as Response);

    const doc = {
      children: [
        { kind: { type: 'Image', src: 'https://example.com/missing.png' } },
      ],
    };
    await expect(resolveImageSources(doc)).rejects.toThrow('Failed to fetch image');
  });

  it('traverses nested children', async () => {
    const mockFetch = vi.mocked(globalThis.fetch);
    mockFetch.mockResolvedValue({
      ok: true,
      arrayBuffer: () => Promise.resolve(new ArrayBuffer(4)),
      headers: new Headers({ 'content-type': 'image/jpeg' }),
    } as Response);

    const doc = {
      children: [
        {
          kind: { type: 'View' },
          children: [
            { kind: { type: 'Image', src: 'https://example.com/nested.jpg' } },
          ],
        },
      ],
    };
    await resolveImageSources(doc);

    expect(mockFetch).toHaveBeenCalledWith('https://example.com/nested.jpg');
  });
});
