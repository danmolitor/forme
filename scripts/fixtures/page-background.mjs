import { readFileSync } from 'node:fs';

// Page backgrounds are painted outside any element: the color (#189) and
// the background image must be marked /Artifact in tagged output, or a
// PDF/UA validator reports untagged content (7.1-3). The image was drawn
// unmarked, and the corpus had no page background to catch it.
export function pageBackgroundDocument() {
  const font = readFileSync(
    new URL('../../packages/fonts-standard/fonts/LiberationSans-Regular.ttf', import.meta.url),
  ).toString('base64');
  // A 1x1 PNG.
  const png =
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGP4z8AAAAMBAQDJ/pLvAAAAAElFTkSuQmCC';
  return {
    metadata: { title: 'Page backgrounds', lang: 'en-US' },
    fonts: [{ family: 'Liberation Sans', weight: 400, src: font }],
    defaultStyle: { fontFamily: 'Liberation Sans', fontSize: 11 },
    children: [
      {
        kind: {
          type: 'Page',
          config: { backgroundImage: `data:image/png;base64,${png}`, backgroundSize: 'cover' },
        },
        style: { backgroundColor: { r: 18 / 255, g: 52 / 255, b: 86 / 255, a: 1 } },
        children: [
          { kind: { type: 'Text', content: 'A page with a background color and a background image.' }, style: {}, children: [] },
        ],
      },
    ],
  };
}
