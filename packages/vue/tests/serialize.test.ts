/**
 * Serialization behavior — mirrors @formepdf/svelte's serialize suite:
 * interpolation and control flow, error propagation, and font handling.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { serialize, render, renderToObject, Font } from '../src/index.js';
// @ts-expect-error .vue fixtures have no type declarations in tests
import HelloWorld from './fixtures/hello-world.vue';
// @ts-expect-error .vue fixtures have no type declarations in tests
import BadProp from './fixtures/bad-prop.vue';
// @ts-expect-error .vue fixtures have no type declarations in tests
import Fonts from './fixtures/fonts.vue';

describe('serialize', () => {
  it('serializes a template with interpolation, v-for, and v-if', async () => {
    const doc = await serialize(HelloWorld, {
      props: { name: 'Vue', items: ['a', 'b'], showFooter: true },
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
    expect(view.children.map((c: { kind: unknown }) => c.kind)).toEqual([
      { type: 'Text', content: 'Hello Vue!' },
      { type: 'Text', content: 'Item: a' },
      { type: 'Text', content: 'Item: b' },
      { type: 'Text', content: 'The footer' },
    ]);
    expect(view.children[0].style).toEqual({ fontSize: 24 });
  });

  it('honors v-if=false and empty v-for', async () => {
    const doc = await serialize(HelloWorld, { props: {} });
    const view = doc.children[0].children[0];
    expect(view.children.map((c: { kind: unknown }) => c.kind)).toEqual([
      { type: 'Text', content: 'Hello World!' },
    ]);
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

  it('document fonts merge with (and override) global fonts by family:weight:italic', async () => {
    Font.register({ family: 'Inter', src: 'global.ttf' });
    Font.register({ family: 'Other', src: 'other.ttf' });
    const doc = await serialize(Fonts, {
      props: { fonts: [{ family: 'Inter', src: 'document.ttf' }] },
    });
    const inter = (doc.fonts as { family: string; src: string; weight: number }[]).filter(
      (f) => f.family === 'Inter' && f.weight === 400,
    );
    expect(inter).toEqual([{ family: 'Inter', src: 'document.ttf', weight: 400, italic: false }]);
    expect(doc.fonts).toContainEqual({ family: 'Other', src: 'other.ttf', weight: 400, italic: false });
  });
});
