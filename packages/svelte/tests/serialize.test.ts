import { describe, it, expect, afterEach } from 'vitest';
import { serialize, render, renderToObject, Font, StyleSheet } from '../src/index.js';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import HelloWorld from './fixtures/hello-world.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import BadProp from './fixtures/bad-prop.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import Fonts from './fixtures/fonts.svelte';

describe('serialize', () => {
  it('serializes a template with interpolation, #each, and #if', async () => {
    const doc = await serialize(HelloWorld, {
      props: { name: 'Svelte', items: ['a', 'b'], showFooter: true },
    });

    expect(doc.metadata).toEqual({ title: 'Hello' });
    expect(doc.children).toHaveLength(1);

    const page = doc.children[0];
    expect(page.kind).toEqual({
      type: 'Page',
      config: {
        size: 'A4',
        margin: { top: 40, right: 40, bottom: 40, left: 40 },
        wrap: true,
      },
    });

    const view = page.children[0];
    expect(view.kind).toEqual({ type: 'View' });
    expect(view.style).toEqual({ flexDirection: 'Column', gap: 8 });
    expect(view.children.map(c => c.kind)).toEqual([
      { type: 'Text', content: 'Hello Svelte!' },
      { type: 'Text', content: 'Item: a' },
      { type: 'Text', content: 'Item: b' },
      { type: 'Text', content: 'The footer' },
    ]);
    expect(view.children[0].style).toEqual({ fontSize: 24 });
  });

  it('honors #if=false and empty #each', async () => {
    const doc = await serialize(HelloWorld, { props: {} });
    const view = doc.children[0].children[0];
    expect(view.children.map(c => c.kind)).toEqual([{ type: 'Text', content: 'Hello World!' }]);
  });

  it('render returns a JSON string and renderToObject the same document', async () => {
    const [str, obj, doc] = await Promise.all([
      render(HelloWorld, { props: { name: 'X' } }),
      renderToObject(HelloWorld, { props: { name: 'X' } }),
      serialize(HelloWorld, { props: { name: 'X' } }),
    ]);
    expect(typeof str).toBe('string');
    expect(JSON.parse(str)).toEqual(obj);
    expect(obj).toEqual(doc);
  });

  it('propagates encoding errors naming component and prop', async () => {
    await expect(serialize(BadProp)).rejects.toThrow(/\[Forme\] <Text>: prop "style"/);
  });
});

describe('font registration and merging', () => {
  afterEach(() => {
    Font.clear();
  });

  it('global Font.register() fonts are included in the serialized document', async () => {
    Font.register({ family: 'Inter', src: 'data:font/ttf;base64,AAAA' });
    const doc = await serialize(HelloWorld);
    expect(doc.fonts).toEqual([
      { family: 'Inter', src: 'data:font/ttf;base64,AAAA', weight: 400, italic: false },
    ]);
  });

  it('the fonts prop is included with sources passed through unresolved', async () => {
    const doc = await serialize(Fonts, {
      props: { fonts: [{ family: 'Roboto', src: 'roboto.ttf', fontWeight: 700 }] },
    });
    expect(doc.fonts).toEqual([
      { family: 'Roboto', src: 'roboto.ttf', weight: 700, italic: false },
    ]);
  });

  it('document fonts override globals on the family:weight:italic key', async () => {
    Font.register({ family: 'Inter', src: 'global.ttf' });
    const doc = await serialize(Fonts, {
      props: { fonts: [{ family: 'Inter', src: 'document.ttf' }] },
    });
    expect(doc.fonts).toEqual([
      { family: 'Inter', src: 'document.ttf', weight: 400, italic: false },
    ]);
  });

  it('globals and document fonts merge when keys differ', async () => {
    Font.register({ family: 'Inter', src: 'inter.ttf' });
    const doc = await serialize(Fonts, {
      props: { fonts: [{ family: 'Roboto', src: 'roboto.ttf' }] },
    });
    expect(doc.fonts!.map(f => f.family).sort()).toEqual(['Inter', 'Roboto']);
  });

  it('oblique font style serializes as italic', async () => {
    const doc = await serialize(Fonts, {
      props: { fonts: [{ family: 'Slant', src: 'slant.ttf', fontStyle: 'oblique' }] },
    });
    expect(doc.fonts![0].italic).toBe(true);
  });

  it('byte-array sources survive serialization as Uint8Array', async () => {
    const bytes = new Uint8Array([0x00, 0x01, 0x02, 0x80, 0xfe, 0xff]);
    const doc = await serialize(Fonts, {
      props: { fonts: [{ family: 'Bytes', src: bytes }] },
    });
    expect(doc.fonts![0].src).toBeInstanceOf(Uint8Array);
    expect(doc.fonts![0].src).toEqual(bytes);
  });

  it('documents without fonts have no fonts key', async () => {
    const doc = await serialize(HelloWorld);
    expect(doc.fonts).toBeUndefined();
  });
});

describe('StyleSheet', () => {
  it('create() returns the styles object with its keys intact', () => {
    const styles = StyleSheet.create({
      heading: { fontSize: 24, fontWeight: 700 },
      body: { color: '#333333' },
    });
    expect(styles.heading).toEqual({ fontSize: 24, fontWeight: 700 });
    expect(styles.body).toEqual({ color: '#333333' });
  });
});
