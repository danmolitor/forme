import { describe, it, expect, vi } from 'vitest';
import { uint8ArrayToBase64 } from '../src/shared/browserHelpers';

const cases = [
  { name: 'empty input', bytes: new Uint8Array(), expected: '' },
  { name: 'one byte', bytes: new Uint8Array([0]), expected: 'AA==' },
  { name: 'two bytes', bytes: new Uint8Array([0, 1]), expected: 'AAE=' },
  { name: 'three bytes', bytes: new Uint8Array([0, 1, 2]), expected: 'AAEC' },
  { name: 'all byte values', bytes: Uint8Array.from({ length: 256 }, (_, i) => i) },
  { name: 'a subarray', bytes: new Uint8Array([9, 0, 1, 254, 255, 9]).subarray(1, 5), expected: 'AAH+/w==' },
  { name: 'large input', bytes: Uint8Array.from({ length: 131073 }, (_, i) => i % 256) },
];

describe('uint8ArrayToBase64', () => {
  it.each(cases)('encodes $name', ({ bytes, expected }) => {
    expect(uint8ArrayToBase64(bytes)).toBe(expected ?? Buffer.from(bytes).toString('base64'));
  });

  it.each(cases)('encodes $name without native support', ({ bytes, expected }) => {
    const input = new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    Object.defineProperty(input, 'toBase64', { value: undefined });
    expect(uint8ArrayToBase64(input)).toBe(expected ?? Buffer.from(bytes).toString('base64'));
  });

  it('uses the native method with the original view as its receiver', () => {
    const bytes = new Uint8Array([9, 0, 1, 2, 9]).subarray(1, 4);
    const toBase64 = vi.fn(function (this: Uint8Array) {
      expect(this).toBe(bytes);
      return 'AAEC';
    });
    Object.defineProperty(bytes, 'toBase64', { value: toBase64 });

    expect(uint8ArrayToBase64(bytes)).toBe('AAEC');
    expect(toBase64).toHaveBeenCalledExactlyOnceWith();
  });

  it('does not reuse stale output when a buffer is mutated', () => {
    const bytes = new Uint8Array([0]);
    expect(uint8ArrayToBase64(bytes)).toBe('AA==');
    bytes[0] = 255;
    expect(uint8ArrayToBase64(bytes)).toBe('/w==');
  });

  it('falls back when toBase64 is not a function', () => {
    const bytes = new Uint8Array([0]);
    Object.defineProperty(bytes, 'toBase64', { value: 'not a function' });
    expect(uint8ArrayToBase64(bytes)).toBe('AA==');
  });
});
