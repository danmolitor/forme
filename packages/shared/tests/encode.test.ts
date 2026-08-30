import { describe, it, expect } from 'vitest';
import { encodeProps, reviveBytesMarker } from '../src/encode.js';

describe('encodeProps', () => {
  it('drops undefined props but keeps falsy values', () => {
    const json = encodeProps('View', { style: undefined, wrap: false, gap: 0, label: '' });
    expect(JSON.parse(json)).toEqual({ wrap: false, gap: 0, label: '' });
  });

  it('round-trips nested style objects', () => {
    const style = { flexDirection: 'row', padding: [8, 16], border: '1px solid #000' };
    const json = encodeProps('View', { style });
    expect(JSON.parse(json)).toEqual({ style });
  });

  it('rejects function props, naming component and prop', () => {
    expect(() => encodeProps('Text', { style: () => {} })).toThrow(
      /\[Forme\] <Text>: prop "style".*function/
    );
  });

  it('rejects functions nested inside props', () => {
    expect(() => encodeProps('View', { style: { color: '#fff', bad: () => {} } })).toThrow(
      /\[Forme\] <View>: prop "style".*function/
    );
  });

  it('rejects circular props, naming component and prop', () => {
    const circular: Record<string, unknown> = {};
    circular.self = circular;
    expect(() => encodeProps('Page', { style: circular })).toThrow(/\[Forme\] <Page>: prop "style"/);
  });

  it('round-trips Uint8Array values through the bytes marker', () => {
    const bytes = new Uint8Array([0x00, 0x01, 0x02, 0x80, 0xfe, 0xff]);
    const json = encodeProps('Document', { fonts: [{ family: 'Bytes', src: bytes }] });
    const decoded = JSON.parse(json, reviveBytesMarker) as {
      fonts: [{ family: string; src: Uint8Array }];
    };
    expect(decoded.fonts[0].family).toBe('Bytes');
    expect(decoded.fonts[0].src).toBeInstanceOf(Uint8Array);
    expect(decoded.fonts[0].src).toEqual(bytes);
  });

  it('rejects props whose objects already use the reserved bytes-marker key', () => {
    expect(() => encodeProps('View', { data: { __formeBytes: 'user-value' } })).toThrow(
      /\[Forme\] <View>: prop "data".*reserved/
    );
  });

  it('round-trips byte arrays larger than one base64 chunk', () => {
    const bytes = new Uint8Array(100_000).map((_, i) => i % 251);
    const json = encodeProps('Document', { src: bytes });
    const decoded = JSON.parse(json, reviveBytesMarker) as { src: Uint8Array };
    expect(decoded.src).toEqual(bytes);
  });
});
