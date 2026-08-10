/**
 * With @formepdf/core uninstalled, renderDocument must reject with an
 * actionable error naming the package and its install command, while
 * the pure serialization API keeps working (core is an
 * optional peer). The mock factory throws, which is exactly what the
 * dynamic `import('@formepdf/core')` does when the package is absent.
 */
import { describe, it, expect, vi } from 'vitest';
import { render, renderDocument, renderDocumentWithLayout } from '../src/index.js';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import HelloWorld from './fixtures/hello-world.svelte';

vi.mock('@formepdf/core', () => {
  throw new Error("Cannot find package '@formepdf/core'");
});

const helloProps = { name: 'Svelte', items: ['alpha', 'beta'], showFooter: true };

describe('renderDocument without @formepdf/core installed', () => {
  it('rejects with an error naming @formepdf/core and how to install it', async () => {
    await expect(renderDocument(HelloWorld, { props: helloProps })).rejects.toThrow(
      /@formepdf\/core.*npm install @formepdf\/core/s,
    );
    await expect(renderDocumentWithLayout(HelloWorld, { props: helloProps })).rejects.toThrow(
      /@formepdf\/core.*npm install @formepdf\/core/s,
    );
  });

  it('keeps serialize/render working', async () => {
    const json = await render(HelloWorld, { props: helloProps });
    const doc = JSON.parse(json);
    expect(doc.children[0].kind.type).toBe('Page');
  });
});
